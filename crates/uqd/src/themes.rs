//! The user's own themes (spec §7.4): load, re-scan, export, import and delete.
//!
//! Themes live in files under `paths.user_themes_dir`, one folder per theme, never in
//! `state.toml`. uqd reads and writes only real folders and regular files, never through a link,
//! and a delete is never recursive (R3). There is no file watcher (G-F6): uqd reloads right
//! after its own import and delete, and `refresh_themes` compares a metadata-only fingerprint.

use crate::service::{Change, Desktop, DesktopError, Event};
use chrono::Utc;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use uq_core::atomic::atomic_replace_dir;
use uq_core::history::{AutoReason, Origin, PointKind};
use uq_core::state::DEFAULT_THEME;
use uq_core::theme::Variant;
use uq_core::theme::uqtheme::{self, ImportError};
use uq_core::theme::user::{self, MAX_USER_THEMES};

const NOT_IN_DEV: &str = "Not available in a test session.";
const ROOT_IS_LINK: &str = "Your themes folder is a link, so uQ won't write through it.";

fn import_err(e: ImportError) -> DesktopError {
    DesktopError::InvalidArgument(e.to_string())
}

impl Desktop {
    /// Read the user's themes folder into the registry, keeping the active theme's last good
    /// copy if its folder has gone (G-F10). Never fails: a bad theme is skipped with a reason.
    pub(crate) fn load_user_themes(&mut self) {
        let root = self.paths.user_themes_dir.clone();
        // Taken first, so a change made during the scan is seen on the next refresh.
        let fingerprint = user::fingerprint(&root);
        let scan = user::scan(&root);
        for s in &scan.skipped {
            tracing::warn!("user theme skipped: {:?}: {}", s.folder, s.problem);
        }
        self.skipped_themes = scan.skipped.clone();
        let active = self.state.theme.clone();
        self.themes.replace_user(scan, Some(&active));
        self.user_themes = fingerprint;
    }

    /// Reload the user's themes if the folder changed since it was last read. Returns whether
    /// anything changed; nothing is read when it didn't.
    pub fn refresh_themes(&mut self) -> bool {
        if user::fingerprint(&self.paths.user_themes_dir) == self.user_themes {
            return false;
        }
        self.load_user_themes();
        let _ = self.events.send(Event::ThemesChanged);
        true
    }

    /// `{themes: [ThemeInfo + active], skipped: [{folder?, reason}], root_is_link}`.
    pub fn theme_info(&mut self) -> Value {
        self.refresh_themes();
        let active = &self.state.theme;
        let themes: Vec<Value> = self
            .themes
            .infos()
            .into_iter()
            .map(|i| {
                let mut v = serde_json::to_value(i).unwrap_or(Value::Null);
                v["active"] = json!(&i.id == active);
                v
            })
            .collect();
        let skipped: Vec<Value> = self
            .skipped_themes
            .iter()
            .map(|s| {
                let mut v = json!({ "reason": s.problem.to_string() });
                if let Some(f) = &s.folder {
                    v["folder"] = json!(f);
                }
                v
            })
            .collect();
        let root_is_link = std::fs::symlink_metadata(&self.paths.user_themes_dir)
            .is_ok_and(|m| m.file_type().is_symlink());
        json!({ "themes": themes, "skipped": skipped, "root_is_link": root_is_link })
    }

    /// The theme as a `.uqtheme` file (R7): the stored theme, never the appearance overrides;
    /// with `with_look`, also the non-null keys of the current `look` section.
    pub fn export_theme(&self, id: &str, with_look: bool) -> Result<String, DesktopError> {
        let (Some(dark), Some(light)) = (
            self.themes.get(id, Variant::Dark),
            self.themes.get(id, Variant::Light),
        ) else {
            return Err(DesktopError::UnknownTheme(id.to_string()));
        };
        let author = self.themes.info(id).and_then(|i| i.author.clone());
        let look = if with_look {
            let section = self
                .state
                .section("look")
                .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
            let map: Map<String, Value> = section
                .as_object()
                .into_iter()
                .flatten()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            (!map.is_empty()).then_some(map)
        } else {
            None
        };
        Ok(uqtheme::to_text(&uqtheme::from_pair(
            dark,
            light,
            author.as_deref(),
            look,
        )))
    }

    /// Refuse when the themes folder is a link or not a folder (R3); absent is fine.
    fn user_root_ok(&self) -> Result<(), DesktopError> {
        match std::fs::symlink_metadata(&self.paths.user_themes_dir) {
            Ok(m) if m.file_type().is_symlink() => {
                Err(DesktopError::InvalidArgument(ROOT_IS_LINK.into()))
            }
            Ok(m) if !m.is_dir() => Err(DesktopError::InvalidArgument(
                "Your themes folder isn't a folder, so uQ can't add a theme to it.".into(),
            )),
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(DesktopError::Io(format!(
                "Couldn't look at your themes folder: {e}"
            ))),
        }
    }

    /// Add (or with `replace`, replace) a theme from `.uqtheme` text, in R9's order. The theme is
    /// installed, not switched to. Returns its id.
    pub async fn import_theme(
        &mut self,
        text: &str,
        replace: bool,
        with_look: bool,
        origin: Origin,
    ) -> Result<String, DesktopError> {
        if self.dev_session {
            return Err(DesktopError::InvalidArgument(NOT_IN_DEV.into()));
        }
        let theme = uqtheme::parse(text).map_err(import_err)?;
        self.refresh_themes();
        let id = theme.dark.id.clone();
        let folder: PathBuf = self.paths.user_themes_dir.join(&id);
        let known = self.themes.is_user(&id);
        let on_disk = std::fs::symlink_metadata(&folder);
        if on_disk.as_ref().is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(DesktopError::InvalidArgument(
                "A link is in the way of this theme's folder, so uQ won't write there.".into(),
            ));
        }
        if (known || on_disk.is_ok()) && !replace {
            return Err(DesktopError::Conflict(format!(
                "You already have a theme called {}. Replace it?",
                theme.name
            )));
        }
        if !known && self.themes.infos().iter().filter(|i| i.user).count() >= MAX_USER_THEMES {
            return Err(DesktopError::InvalidArgument(
                user::Problem::TooMany.to_string(),
            ));
        }
        self.user_root_ok()?;
        // Replacing swaps the whole folder, so anything else in it would be deleted.
        if on_disk.as_ref().is_ok_and(|m| m.is_dir()) && has_other_files(&folder) {
            return Err(DesktopError::InvalidArgument(
                "This theme's folder holds other files. Move them out first.".into(),
            ));
        }
        if let Err(e) = self.restore.save(
            &self.state,
            &format!("Before adding the {} theme", theme.name),
            PointKind::Auto(AutoReason::BeforeImport),
            Utc::now().timestamp(),
        ) {
            tracing::warn!("saving before-import point: {e}");
        }
        atomic_replace_dir(&folder, &uqtheme::install_files(&theme))
            .map_err(|e| DesktopError::Io(format!("Couldn't save the theme: {e}")))?;
        self.load_user_themes();
        let res = self.after_import(&theme, with_look, origin).await;
        let _ = self.events.send(Event::ThemesChanged);
        res.map(|()| id)
    }

    /// Steps 9 and 10 of R9: re-render a replaced active theme, then the optional `look`.
    async fn after_import(
        &mut self,
        theme: &uqtheme::UqTheme,
        with_look: bool,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        let id = &theme.dark.id;
        if !self.themes.is_user(id) {
            return Err(DesktopError::Io(
                "The theme was saved, but uQ couldn't load it.".into(),
            ));
        }
        if &self.state.theme == id {
            self.commit(self.state.clone(), Change::Theme, origin.clone())
                .await
                .map_err(|e| {
                    DesktopError::Io(format!(
                        "The theme was added, but it couldn't be applied: {e}"
                    ))
                })?;
        }
        let Some(look) = theme.look.as_ref().filter(|_| with_look) else {
            return Ok(());
        };
        let failed = |reason: String| {
            DesktopError::InvalidSetting(format!(
                "The theme was added, but its window look couldn't be used: {reason}"
            ))
        };
        let mut merged = self
            .state
            .section("look")
            .map_err(|e| failed(e.to_string()))?;
        if let Some(obj) = merged.as_object_mut() {
            obj.extend(look.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        self.set_section("look", &merged.to_string(), origin)
            .await
            .map_err(|e| failed(e.to_string()))
    }

    /// Remove a theme of the user's (R12). Only `dark.toml` and `light.toml` go, then the folder
    /// if that leaves it empty; nothing is followed through a link. When it's the active theme,
    /// the default theme is switched to first. Returns `{switched, profiles}`.
    pub async fn delete_theme(&mut self, id: &str, origin: Origin) -> Result<Value, DesktopError> {
        if self.dev_session {
            return Err(DesktopError::InvalidArgument(NOT_IN_DEV.into()));
        }
        self.refresh_themes();
        match self.themes.info(id) {
            None => return Err(DesktopError::UnknownTheme(id.to_string())),
            Some(i) if !i.user => {
                return Err(DesktopError::InvalidArgument(
                    "Themes that come with uQ can't be deleted.".into(),
                ));
            }
            Some(_) => {}
        }
        if !user::valid_user_id(id) {
            return Err(DesktopError::UnknownTheme(id.to_string()));
        }
        self.user_root_ok()?;
        if std::fs::symlink_metadata(self.paths.user_themes_dir.join(id))
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(DesktopError::InvalidArgument(
                "This theme's folder is a link, so uQ won't remove it.".into(),
            ));
        }
        let profiles: Vec<String> = self
            .profiles
            .list()
            .into_iter()
            .filter(|p| {
                p.values
                    .get("appearance")
                    .and_then(|a| a.get("theme"))
                    .and_then(Value::as_str)
                    == Some(id)
            })
            .map(|p| p.name)
            .collect();
        let switched = self.state.theme == id;
        if switched {
            self.set_theme(DEFAULT_THEME, self.state.variant, origin)
                .await?;
        }
        let folder = self.paths.user_themes_dir.join(id);
        // Checked again right before removing, so a folder swapped for a link is left alone.
        let is_real_dir = std::fs::symlink_metadata(&folder).is_ok_and(|m| m.is_dir());
        let mut failed = None;
        if is_real_dir {
            for v in Variant::ALL {
                match std::fs::remove_file(folder.join(format!("{v}.toml"))) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        failed = Some(DesktopError::Io(format!("Couldn't remove the theme: {e}")));
                        break;
                    }
                }
            }
            if failed.is_none() {
                // Not recursive: a folder that holds anything else stays.
                let _ = std::fs::remove_dir(&folder);
            }
        }
        self.load_user_themes();
        let _ = self.events.send(Event::ThemesChanged);
        if let Some(e) = failed {
            return Err(e);
        }
        Ok(json!({ "switched": switched, "profiles": profiles }))
    }
}

/// Whether `folder` holds anything besides the two variant files.
fn has_other_files(folder: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(folder) else {
        return false;
    };
    rd.flatten().any(|e| {
        let name = e.file_name();
        name != "dark.toml" && name != "light.toml"
    })
}

#[cfg(test)]
mod tests;
