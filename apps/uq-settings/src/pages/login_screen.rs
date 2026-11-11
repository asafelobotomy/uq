//! The Login screen page (Phase 5 §4.1): how uQ's login screen looks, and auto-login. Every
//! write goes through `uq-settings-helper` (`org.uq.SettingsHelper1`), which checks polkit and
//! writes one fixed file. The page reads those files back to show what is set.
//!
//! "Same as mine" choices (theme, wallpaper, display layout) are copied when saved; later
//! changes to the user's own desktop don't follow.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::client::DesktopProxy;
use uq_core::state::DEFAULT_THEME;

use super::proxies::{HELPER_NAME, HelperProxy};
use super::runner::{PageLogic, parse_json};
use super::sysbus::{self, HELPER, SystemBus, failure, settle};

/// uQ's greetd config: the page is shown only when it exists (uQ is the login screen).
pub const GREETD_TOML: &str = "etc/uq/greetd.toml";
pub const GREETER_TOML: &str = "etc/uq/greeter.toml";
pub const GREETER_MONITORS: &str = "etc/uq/greeter-monitors.toml";
pub const GREETER_WALLPAPER: &str = "var/lib/uq-greeter/wallpaper";
pub const RENDERED: &str = "usr/share/uq/themes/rendered";
/// The largest picture the login screen takes (Phase 5 §4.1).
pub const MAX_WALLPAPER: u64 = 20 * 1024 * 1024;
/// Read at most this much of a config file.
const MAX_CONFIG: u64 = 256 * 1024;

pub const SAVED: &str = "Saved. The login screen uses it the next time it shows.";

/// Whether uQ is the login screen on this machine (`/etc/uq/greetd.toml` exists).
pub fn greeter_installed() -> bool {
    Path::new("/").join(GREETD_TOML).is_file()
}

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    /// `theme` "" means "same as mine": the user's current theme and variant, copied now.
    SetAppearance {
        theme: String,
        variant: String,
        hide_users: bool,
        reduce_motion: bool,
    },
    /// Copy the user's own wallpaper.
    WallpaperMine,
    /// Copy a picked picture.
    WallpaperFile {
        path: PathBuf,
    },
    ClearWallpaper,
    /// Copy the user's display layout.
    UseMyDisplays,
    /// Auto-login for the current user (`true`) or nobody.
    SetAutoLogin {
        on: bool,
    },
}

pub struct LoginScreenState {
    session: Option<zbus::Connection>,
    system: SystemBus,
    /// `/`, or a temporary root in tests.
    root: PathBuf,
    /// The login name of the user running the app.
    me: String,
    write: Mutex<()>,
    notice: StdMutex<(u64, String)>,
}

impl LoginScreenState {
    pub fn new(session: zbus::Connection) -> Self {
        let me = std::env::var("USER").unwrap_or_default();
        Self::with(Some(session), SystemBus::system(), PathBuf::from("/"), me)
    }

    pub fn with(
        session: Option<zbus::Connection>,
        system: SystemBus,
        root: PathBuf,
        me: String,
    ) -> Self {
        Self {
            session,
            system,
            root,
            me,
            write: Mutex::new(()),
            notice: StdMutex::new((0, String::new())),
        }
    }

    fn at(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn notice(&self, text: &str) {
        let mut n = self.notice.lock().unwrap();
        *n = (n.0 + 1, text.to_string());
    }

    async fn helper(&self) -> zbus::Result<HelperProxy<'static>> {
        let conn = self.system.conn().await?;
        HelperProxy::new(&conn).await
    }

    async fn uqd(&self) -> Option<DesktopProxy<'static>> {
        let conn = self.session.as_ref()?;
        DesktopProxy::new(conn).await.ok()
    }

    /// The user's current theme and variant, when uqd answers.
    async fn my_theme(&self) -> Option<(String, String)> {
        let proxy = self.uqd().await?;
        sysbus::within(proxy.get_theme()).await.ok()
    }

    /// The picture on the user's desktop: the all-screens entry, else the first screen's.
    async fn my_wallpaper(&self) -> Option<PathBuf> {
        let proxy = self.uqd().await?;
        let section =
            parse_json(&sysbus::within(proxy.get_section("wallpaper")).await.ok()?).ok()?;
        wallpaper_image(&section)
    }

    async fn my_monitors(&self) -> Option<Value> {
        let proxy = self.uqd().await?;
        let section =
            parse_json(&sysbus::within(proxy.get_section("monitors")).await.ok()?).ok()?;
        Some(json!({"outputs": section.get("outputs").cloned().unwrap_or(json!({}))}))
    }

    fn variants(&self) -> Vec<(String, String)> {
        rendered_variants(&self.at(RENDERED))
    }

    async fn set_appearance(
        &self,
        theme: &str,
        variant: &str,
        hide_users: bool,
        reduce_motion: bool,
    ) -> zbus::Result<()> {
        let (theme, variant) = if theme.is_empty() {
            self.my_theme().await.ok_or_else(|| {
                failure("Your own theme couldn't be read because uQ's settings service isn't running. Pick a theme from the list.")
            })?
        } else {
            (theme.to_string(), variant.to_string())
        };
        if !self.variants().contains(&(theme.clone(), variant.clone())) {
            return Err(failure(format!(
                "The {theme} theme ({variant}) isn't installed for the login screen. Pick one from the list."
            )));
        }
        let helper = self.helper().await?;
        let r = helper
            .set_greeter_appearance(&theme, &variant, hide_users, reduce_motion)
            .await;
        self.saved(r)
    }

    async fn set_wallpaper(&self, path: &Path) -> zbus::Result<()> {
        let file = open_wallpaper(path)?;
        let helper = self.helper().await?;
        let r = helper
            .set_greeter_wallpaper(zbus::zvariant::Fd::from(&file))
            .await;
        self.saved(r)
    }

    fn saved(&self, r: zbus::Result<()>) -> zbus::Result<()> {
        let ok = r.is_ok();
        settle(r, &HELPER)?;
        if ok {
            self.notice(SAVED);
        }
        Ok(())
    }

    fn read_greeter(&self) -> Value {
        let table = read_toml(&self.at(GREETER_TOML)).unwrap_or_default();
        let s = |k: &str| table.get(k).and_then(|v| v.as_str()).map(str::to_string);
        let b = |k: &str| table.get(k).and_then(toml::Value::as_bool).unwrap_or(false);
        json!({
            "theme": s("theme").unwrap_or_else(|| DEFAULT_THEME.to_string()),
            "variant": s("variant").unwrap_or_else(|| "dark".to_string()),
            "hide_users": b("hide_users"),
            "reduce_motion": b("reduce_motion"),
        })
    }

    fn auto_login_user(&self) -> String {
        auto_login_user(&self.root)
    }
}

/// `initial_session.user` in uQ's greetd config under `root`, or "".
pub fn auto_login_user(root: &Path) -> String {
    read_toml(&root.join(GREETD_TOML))
        .and_then(|t| {
            t.get("initial_session")?
                .get("user")?
                .as_str()
                .map(str::to_string)
        })
        .unwrap_or_default()
}

/// A small TOML file, or `None` when missing, too large or unreadable.
fn read_toml(path: &Path) -> Option<toml::Table> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_CONFIG {
        return None;
    }
    std::fs::read_to_string(path).ok()?.parse().ok()
}

/// The `(theme, variant)` pairs shipped pre-rendered under `dir`, sorted; only dark and light,
/// and only those with a `tokens.json`.
pub fn rendered_variants(dir: &Path) -> Vec<(String, String)> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = rd
        .flatten()
        .filter(|e| e.path().join("tokens.json").is_file())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let (theme, variant) = name.rsplit_once('-')?;
            let ok_theme = !theme.is_empty()
                && theme.len() <= 40
                && theme
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
            (ok_theme && matches!(variant, "dark" | "light"))
                .then(|| (theme.to_string(), variant.to_string()))
        })
        .collect();
    out.sort();
    out
}

/// The picture a `wallpaper` section shows: `outputs["*"].image`, else the first output's.
pub fn wallpaper_image(section: &Value) -> Option<PathBuf> {
    let outputs = section.get("outputs")?.as_object()?;
    let image = |v: &Value| v.get("image").and_then(Value::as_str).map(PathBuf::from);
    outputs
        .get("*")
        .and_then(image)
        .or_else(|| outputs.values().find_map(image))
}

/// Open a picture to hand to the helper: a regular file of at most 20 MiB.
pub fn open_wallpaper(path: &Path) -> zbus::Result<File> {
    let file =
        File::open(path).map_err(|e| failure(format!("That picture couldn't be opened: {e}")))?;
    let meta = file
        .metadata()
        .map_err(|e| failure(format!("That picture couldn't be read: {e}")))?;
    if !meta.is_file() {
        return Err(failure("That isn't a picture file."));
    }
    if meta.len() > MAX_WALLPAPER {
        return Err(failure(
            "That picture is larger than 20 MiB. Pick a smaller one.",
        ));
    }
    Ok(file)
}

impl PageLogic for LoginScreenState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        match op {
            Op::Refresh => Ok(()),
            Op::SetAppearance {
                theme,
                variant,
                hide_users,
                reduce_motion,
            } => {
                self.set_appearance(&theme, &variant, hide_users, reduce_motion)
                    .await
            }
            Op::WallpaperMine => {
                let path = self.my_wallpaper().await.ok_or_else(|| {
                    failure("Your desktop has no single picture to copy. Pick a picture instead.")
                })?;
                self.set_wallpaper(&path).await
            }
            Op::WallpaperFile { path } => self.set_wallpaper(&path).await,
            Op::ClearWallpaper => {
                let r = self.helper().await?.clear_greeter_wallpaper().await;
                self.saved(r)
            }
            Op::UseMyDisplays => {
                let monitors = self.my_monitors().await.ok_or_else(|| {
                    failure("Your display layout couldn't be read because uQ's settings service isn't running.")
                })?;
                let r = self
                    .helper()
                    .await?
                    .set_greeter_monitors(&monitors.to_string())
                    .await;
                self.saved(r)
            }
            Op::SetAutoLogin { on } => {
                if on && self.me.is_empty() {
                    return Err(failure("Your user name couldn't be found."));
                }
                let user = if on { self.me.as_str() } else { "" };
                let r = self.helper().await?.set_auto_login(user).await;
                let ok = r.is_ok();
                settle(r, &HELPER)?;
                if ok {
                    self.notice(if on {
                        "Auto-login is on. It takes effect the next time the computer starts."
                    } else {
                        "Auto-login is off."
                    });
                }
                Ok(())
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let helper = match self.system.conn().await {
            Ok(c) => sysbus::has_service(&c, HELPER_NAME).await,
            Err(_) => false,
        };
        let mine = self
            .my_theme()
            .await
            .map(|(t, v)| {
                let available = self.variants().contains(&(t.clone(), v.clone()));
                json!({"theme": t, "variant": v, "available": available})
            })
            .unwrap_or(Value::Null);
        let my_wallpaper = self.my_wallpaper().await;
        let variants: Vec<Value> = self
            .variants()
            .into_iter()
            .map(|(t, v)| json!({"id": format!("{t}-{v}"), "theme": t, "variant": v}))
            .collect();
        let (seq, text) = self.notice.lock().unwrap().clone();
        Ok(json!({
            "installed": self.at(GREETD_TOML).is_file(),
            "helper": helper,
            "helper_note": HELPER.missing_text(),
            "greeter": self.read_greeter(),
            "variants": variants,
            "mine": mine,
            "wallpaper": {
                "set": self.at(GREETER_WALLPAPER).is_file(),
                "mine": my_wallpaper.map(|p| p.display().to_string()).unwrap_or_default(),
            },
            "monitors": {"set": self.at(GREETER_MONITORS).is_file()},
            "auto_login": {"user": self.auto_login_user(), "me": self.me},
            "notice": {"seq": seq, "text": text},
        }))
    }
}

#[cfg(test)]
mod tests;
