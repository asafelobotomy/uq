//! The fake uqd's themes: two system themes (the real files) and the user's, in memory. Imports
//! go through the **real** `uqtheme::parse`, so the fake refuses what uqd refuses, and the same
//! plain messages come back.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};
use uq_core::theme::uqtheme::{self, UqTheme};
use uq_core::theme::user::{self, MAX_USER_THEMES};
use uq_core::theme::{Theme, Variant};

use super::{FakeError, FakeUqd, invalid};

/// What the fake switches to when the active theme is deleted. (uqd's own default isn't one of
/// the two themes the fake holds.)
pub const DEFAULT_THEME: &str = "gruvbox";

#[derive(Clone)]
pub struct SystemTheme {
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    pub dark: Theme,
    pub light: Theme,
}

fn system(id: &str, dark: &str, light: &str) -> SystemTheme {
    let (d, author) = Theme::from_toml_with_author(dark, id, Variant::Dark).unwrap();
    let (l, _) = Theme::from_toml_with_author(light, id, Variant::Light).unwrap();
    SystemTheme {
        id: id.to_string(),
        name: d.name.clone(),
        author,
        dark: d,
        light: l,
    }
}

#[derive(Clone)]
pub struct FakeThemes {
    pub system: Arc<Vec<SystemTheme>>,
    /// By id (`user-<slug>`).
    pub user: Arc<Mutex<BTreeMap<String, UqTheme>>>,
    /// The theme calls made, by D-Bus method name, in order.
    pub calls: Arc<Mutex<Vec<String>>>,
    /// What `ListThemeInfo` says was skipped (`{folder, reason}`).
    pub skipped: Arc<Mutex<Vec<Value>>>,
    /// When set, the next `ImportTheme`, `ExportTheme` or `DeleteTheme` fails with
    /// `InvalidArgument` and this text.
    pub refuse_next: Arc<Mutex<Option<String>>>,
}

impl Default for FakeThemes {
    fn default() -> Self {
        Self {
            system: Arc::new(vec![
                system(
                    "gruvbox",
                    include_str!("../../../../../themes/gruvbox/dark.toml"),
                    include_str!("../../../../../themes/gruvbox/light.toml"),
                ),
                system(
                    "dracula",
                    include_str!("../../../../../themes/dracula/dark.toml"),
                    include_str!("../../../../../themes/dracula/light.toml"),
                ),
            ]),
            user: Arc::default(),
            calls: Arc::default(),
            skipped: Arc::default(),
            refuse_next: Arc::default(),
        }
    }
}

impl FakeThemes {
    fn log(&self, call: &str) -> Result<(), FakeError> {
        self.calls.lock().unwrap().push(call.to_string());
        match self.refuse_next.lock().unwrap().take() {
            Some(text) => Err(FakeError::InvalidArgument(text)),
            None => Ok(()),
        }
    }

    /// The calls made so far.
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    /// The system theme `id`.
    pub fn system_theme(&self, id: &str) -> Option<&SystemTheme> {
        self.system.iter().find(|t| t.id == id)
    }

    /// `ListThemes`: the system ids, then the user's.
    pub fn ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.system.iter().map(|t| t.id.clone()).collect();
        ids.extend(self.user.lock().unwrap().keys().cloned());
        ids
    }

    /// Add a user theme from `.uqtheme` text, as if its folder had been copied in. Returns its id.
    pub fn add(&self, text: &str) -> String {
        let t = uqtheme::parse(text).expect("a valid theme file");
        let id = t.dark.id.clone();
        self.user.lock().unwrap().insert(id.clone(), t);
        id
    }

    /// The system theme `id` (or user theme) as `.uqtheme` text, with no window look.
    pub fn text_of(&self, id: &str) -> Option<String> {
        self.pair(id).map(|(dark, light, author)| {
            uqtheme::to_text(&uqtheme::from_pair(&dark, &light, author.as_deref(), None))
        })
    }

    fn pair(&self, id: &str) -> Option<(Theme, Theme, Option<String>)> {
        if let Some(t) = self.system_theme(id) {
            return Some((t.dark.clone(), t.light.clone(), t.author.clone()));
        }
        let user = self.user.lock().unwrap();
        user.get(id)
            .map(|t| (t.dark.clone(), t.light.clone(), t.author.clone()))
    }

    /// `ListThemeInfo`.
    pub fn info(&self, active: &str) -> String {
        let mut rows: Vec<(bool, String, String, Value)> = Vec::new();
        for t in self.system.iter() {
            rows.push((
                false,
                t.name.to_lowercase(),
                t.id.clone(),
                json!({
                    "id": t.id, "name": t.name, "author": t.author, "user": false,
                    "missing": false, "active": t.id == active,
                }),
            ));
        }
        for (id, t) in self.user.lock().unwrap().iter() {
            rows.push((
                true,
                t.name.to_lowercase(),
                id.clone(),
                json!({
                    "id": id, "name": t.name, "author": t.author, "user": true,
                    "missing": false, "active": id == active,
                }),
            ));
        }
        rows.sort_by(|a, b| (a.0, &a.1, &a.2).cmp(&(b.0, &b.1, &b.2)));
        let themes: Vec<Value> = rows.into_iter().map(|r| r.3).collect();
        json!({
            "themes": themes,
            "skipped": *self.skipped.lock().unwrap(),
            "root_is_link": false,
        })
        .to_string()
    }

    /// `ExportTheme`.
    pub fn export(&self, uqd: &FakeUqd, id: &str, with_look: bool) -> Result<String, FakeError> {
        self.log("ExportTheme")?;
        let Some((dark, light, author)) = self.pair(id) else {
            return Err(FakeError::UnknownTheme(id.to_string()));
        };
        let look = if with_look {
            let section = uqd.state.lock().unwrap().section("look").map_err(invalid)?;
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
            &dark,
            &light,
            author.as_deref(),
            look,
        )))
    }

    /// `ImportTheme`: not switched to; with `with_look`, the file's look is merged over the
    /// current one as `origin`'s change.
    pub fn import(
        &self,
        uqd: &FakeUqd,
        text: &str,
        replace: bool,
        with_look: bool,
        origin: &str,
    ) -> Result<String, FakeError> {
        self.log("ImportTheme")?;
        let theme = uqtheme::parse(text).map_err(|e| FakeError::InvalidArgument(e.to_string()))?;
        let id = theme.dark.id.clone();
        {
            let user = self.user.lock().unwrap();
            let known = user.contains_key(&id);
            if known && !replace {
                return Err(FakeError::Conflict(format!(
                    "You already have a theme called {}. Replace it?",
                    theme.name
                )));
            }
            if !known && user.len() >= MAX_USER_THEMES {
                return Err(FakeError::InvalidArgument(
                    user::Problem::TooMany.to_string(),
                ));
            }
        }
        if let Some(look) = theme.look.as_ref().filter(|_| with_look) {
            uqd.write(origin, |s| {
                let mut merged = s.section("look").map_err(invalid)?;
                if let Some(obj) = merged.as_object_mut() {
                    obj.extend(look.iter().map(|(k, v)| (k.clone(), v.clone())));
                }
                s.set_section("look", merged).map_err(invalid)
            })?;
        }
        self.user.lock().unwrap().insert(id.clone(), theme);
        Ok(id)
    }

    /// `DeleteTheme`: `{switched, profiles}` as JSON.
    pub fn delete(&self, uqd: &FakeUqd, id: &str, origin: &str) -> Result<String, FakeError> {
        self.log("DeleteTheme")?;
        if self.system_theme(id).is_some() {
            return Err(FakeError::InvalidArgument(
                "Themes that come with uQ can't be deleted.".into(),
            ));
        }
        if !self.user.lock().unwrap().contains_key(id) {
            return Err(FakeError::UnknownTheme(id.to_string()));
        }
        let switched = uqd.state.lock().unwrap().theme == id;
        if switched {
            uqd.write(origin, |s| {
                s.theme = DEFAULT_THEME.to_string();
                Ok(())
            })?;
        }
        let profiles: Vec<String> = uqd
            .profiles
            .store
            .lock()
            .unwrap()
            .values()
            .filter(|p| {
                p.values
                    .get("appearance")
                    .and_then(|a| a.get("theme"))
                    .and_then(Value::as_str)
                    == Some(id)
            })
            .map(|p| p.name.clone())
            .collect();
        self.user.lock().unwrap().remove(id);
        Ok(json!({"switched": switched, "profiles": profiles}).to_string())
    }
}
