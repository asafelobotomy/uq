//! The helper's operations and the `org.uq.SettingsHelper1` D-Bus interface.
//!
//! Every operation runs in the same order: check the arguments (closed sets, patterns, fd
//! type and size), then ask the [`Authority`], then write one fixed file under the write lock.
//! A refused or failed check writes nothing.

use crate::authority::{Action, Authority};
use crate::config::Config;
use crate::idle::Activity;
use crate::passwd::Passwd;
use crate::templates;
use crate::values::{GreeterVariant, Invalid, PowerAction, ThemeName, UID_MAX, UID_MIN, UserName};
use crate::write::{remove, write_atomic};
use crate::{NAME, PATH};
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::fd::OwnedFd;
use std::os::unix::fs::{FileExt, OpenOptionsExt};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use uq_core::settings::Monitors;

/// The largest wallpaper copied (Phase 5 §4.1).
pub const MAX_WALLPAPER: u64 = 20 * 1024 * 1024;
/// How long reading the passed wallpaper fd may take before the call gives up.
pub const WALLPAPER_READ_TIMEOUT: Duration = Duration::from_secs(20);
/// The largest monitors JSON accepted (16 outputs fit in a few KiB).
pub const MAX_MONITORS_JSON: usize = 64 * 1024;
/// The largest `greetd.toml` the helper will read and rewrite.
pub const MAX_GREETD_TOML: u64 = 256 * 1024;

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.uq.SettingsHelper1.Error")]
pub enum HelperError {
    #[zbus(error)]
    ZBus(zbus::Error),
    /// polkit refused, the prompt was cancelled, or the check could not be made.
    NotAuthorized(String),
    /// An argument was outside its closed set or pattern, or the fd was not acceptable.
    InvalidArgument(String),
    /// The write itself failed (the message says which fixed file).
    Failed(String),
}

impl From<Invalid> for HelperError {
    fn from(e: Invalid) -> Self {
        Self::InvalidArgument(e.0)
    }
}

fn failed(what: &Path, e: io::Error) -> HelperError {
    HelperError::Failed(format!("couldn't write {}: {e}", what.display()))
}

/// The operations, independent of D-Bus: `sender` is the caller's unique bus name.
pub struct Helper {
    cfg: Config,
    authority: Arc<dyn Authority>,
    passwd: Arc<dyn Passwd>,
    lock: tokio::sync::Mutex<()>,
    activity: Arc<Activity>,
}

impl Helper {
    pub fn new(cfg: Config, authority: Arc<dyn Authority>, passwd: Arc<dyn Passwd>) -> Self {
        Self {
            cfg,
            authority,
            passwd,
            lock: tokio::sync::Mutex::new(()),
            activity: Arc::new(Activity::default()),
        }
    }

    pub fn activity(&self) -> &Arc<Activity> {
        &self.activity
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    async fn authorize(&self, sender: &str, action: Action) -> Result<(), HelperError> {
        match self.authority.check(sender, action).await {
            Ok(true) => {
                tracing::info!("{} allowed for {sender}", action.id());
                Ok(())
            }
            Ok(false) => {
                tracing::info!("{} refused for {sender}", action.id());
                Err(HelperError::NotAuthorized(format!(
                    "not authorised for {}",
                    action.id()
                )))
            }
            Err(e) => {
                tracing::warn!("{} for {sender}: {e}", action.id());
                Err(HelperError::NotAuthorized(format!(
                    "couldn't check authorisation for {}: {e}",
                    action.id()
                )))
            }
        }
    }

    async fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), HelperError> {
        let _g = self.lock.lock().await;
        write_atomic(path, bytes).map_err(|e| failed(path, e))
    }

    /// `SetLogindPowerActions`: `/etc/systemd/logind.conf.d/50-uq.conf`. logind is never
    /// restarted; the drop-in applies at its next reload.
    pub async fn set_logind_power_actions(
        &self,
        sender: &str,
        lid: &str,
        lid_on_ac: &str,
        power_key: &str,
    ) -> Result<(), HelperError> {
        let lid = PowerAction::parse("lid", lid)?;
        let lid_on_ac = PowerAction::parse("lid_on_ac", lid_on_ac)?;
        let power_key = PowerAction::parse("power_key", power_key)?;
        self.authorize(sender, Action::Power).await?;
        let body = templates::logind(lid, lid_on_ac, power_key);
        self.write(&self.cfg.logind_dropin(), body.as_bytes()).await
    }

    /// `SetGreeterAppearance`: `/etc/uq/greeter.toml`. The theme and variant must name a
    /// shipped pre-rendered variant.
    pub async fn set_greeter_appearance(
        &self,
        sender: &str,
        theme: &str,
        variant: &str,
        hide_users: bool,
        reduce_motion: bool,
    ) -> Result<(), HelperError> {
        let theme = ThemeName::parse(theme)?;
        let variant = GreeterVariant::parse(variant)?;
        let tokens = self.cfg.rendered_tokens(&theme, variant);
        if !std::fs::metadata(&tokens).is_ok_and(|m| m.is_file()) {
            return Err(HelperError::InvalidArgument(format!(
                "{}-{} isn't an installed theme variant",
                theme.as_str(),
                variant.as_str()
            )));
        }
        self.authorize(sender, Action::Greeter).await?;
        let body = templates::greeter(&theme, variant, hide_users, reduce_motion);
        self.write(&self.cfg.greeter_toml(), body.as_bytes()).await
    }

    /// `SetGreeterWallpaper`: copy the passed file byte for byte to
    /// `/var/lib/uq-greeter/wallpaper`. The fd must be a regular file of at most 20 MiB; it is
    /// read with `pread` from offset 0 (the caller's offset is untouched) and never decoded.
    /// The fd was opened by the caller, so the helper can only read what the caller could.
    pub async fn set_greeter_wallpaper(
        &self,
        sender: &str,
        fd: OwnedFd,
    ) -> Result<(), HelperError> {
        let file = File::from(fd);
        check_wallpaper_fd(&file)?;
        self.authorize(sender, Action::Greeter).await?;
        // A caller-controlled file (FUSE, a slow network mount) may stall a read: read off the
        // async threads, with a deadline.
        let read = tokio::task::spawn_blocking(move || read_capped(&file, MAX_WALLPAPER));
        let bytes = match tokio::time::timeout(WALLPAPER_READ_TIMEOUT, read).await {
            Err(_) => {
                return Err(HelperError::InvalidArgument(
                    "reading the wallpaper took too long".into(),
                ));
            }
            Ok(Err(e)) => return Err(HelperError::Failed(format!("wallpaper read: {e}"))),
            Ok(Ok(r)) => r?,
        };
        self.write(&self.cfg.greeter_wallpaper(), &bytes).await
    }

    /// `ClearGreeterWallpaper`: remove `/var/lib/uq-greeter/wallpaper` (the greeter then uses
    /// the theme's background colour).
    pub async fn clear_greeter_wallpaper(&self, sender: &str) -> Result<(), HelperError> {
        self.authorize(sender, Action::Greeter).await?;
        let path = self.cfg.greeter_wallpaper();
        let _g = self.lock.lock().await;
        remove(&path).map_err(|e| failed(&path, e))
    }

    /// `SetGreeterMonitors`: `/etc/uq/greeter-monitors.toml`, from a `monitors` section as
    /// JSON (`{"outputs": {...}}`), validated strictly by `uq_core`.
    pub async fn set_greeter_monitors(&self, sender: &str, json: &str) -> Result<(), HelperError> {
        let monitors = parse_monitors(json)?;
        self.authorize(sender, Action::Greeter).await?;
        let body = templates::greeter_monitors(&monitors);
        self.write(&self.cfg.greeter_monitors(), body.as_bytes())
            .await
    }

    /// `SetAutoLogin`: set (or with `""`, remove) `[initial_session]` in `/etc/uq/greetd.toml`.
    /// The user must exist with a regular uid; the command is always the uQ session.
    pub async fn set_auto_login(&self, sender: &str, user: &str) -> Result<(), HelperError> {
        let user = if user.is_empty() {
            None
        } else {
            Some(self.check_auto_login_user(user)?)
        };
        self.authorize(sender, Action::AutoLogin).await?;
        let path = self.cfg.greetd_toml();
        let _g = self.lock.lock().await;
        let existing = read_config(&path, MAX_GREETD_TOML)?;
        let body = templates::greetd_autologin(&existing, user.as_ref())
            .map_err(|e| HelperError::Failed(e.to_string()))?;
        write_atomic(&path, body.as_bytes()).map_err(|e| failed(&path, e))
    }

    fn check_auto_login_user(&self, user: &str) -> Result<UserName, HelperError> {
        let user = UserName::parse(user)?;
        let uid = self
            .passwd
            .uid_of(user.as_str())
            .map_err(|e| HelperError::Failed(format!("user lookup failed: {e}")))?
            .ok_or_else(|| HelperError::InvalidArgument("no such user".into()))?;
        if !(UID_MIN..=UID_MAX).contains(&uid) {
            return Err(HelperError::InvalidArgument(format!(
                "auto-login is only for regular accounts (uid {UID_MIN} to {UID_MAX})"
            )));
        }
        Ok(user)
    }
}

/// The passed fd must be a regular file within the size cap (`fstat`; directories, FIFOs,
/// sockets and devices are refused before anything is read).
pub fn check_wallpaper_fd(file: &File) -> Result<u64, HelperError> {
    let meta = file
        .metadata()
        .map_err(|e| HelperError::InvalidArgument(format!("can't stat the wallpaper: {e}")))?;
    if !meta.file_type().is_file() {
        return Err(HelperError::InvalidArgument(
            "the wallpaper must be a regular file".into(),
        ));
    }
    if meta.len() > MAX_WALLPAPER {
        return Err(HelperError::InvalidArgument(
            "the wallpaper is larger than 20 MiB".into(),
        ));
    }
    Ok(meta.len())
}

/// Read a whole file with `pread` from offset 0, refusing it once it passes `cap` bytes (it may
/// have grown since the `fstat`).
pub fn read_capped(file: &File, cap: u64) -> Result<Vec<u8>, HelperError> {
    let mut out = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut off = 0u64;
    loop {
        let n = match file.read_at(&mut buf, off) {
            Ok(0) => return Ok(out),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                return Err(HelperError::InvalidArgument(format!(
                    "can't read the wallpaper: {e}"
                )));
            }
        };
        if out.len() as u64 + n as u64 > cap {
            return Err(HelperError::InvalidArgument(
                "the wallpaper is larger than 20 MiB".into(),
            ));
        }
        out.extend_from_slice(&buf[..n]);
        off += n as u64;
    }
}

/// Parse and validate a `monitors` section given as JSON.
pub fn parse_monitors(json: &str) -> Result<Monitors, HelperError> {
    if json.len() > MAX_MONITORS_JSON {
        return Err(HelperError::InvalidArgument(
            "the monitor layout is too large".into(),
        ));
    }
    let v: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| HelperError::InvalidArgument(format!("monitor layout isn't JSON: {e}")))?;
    Monitors::from_section(&v).map_err(|e| HelperError::InvalidArgument(e.to_string()))
}

/// Read one of the helper's own config files: a regular file, not through a symlink, capped.
fn read_config(path: &Path, cap: u64) -> Result<String, HelperError> {
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => HelperError::Failed(format!(
                "{} is missing; is the uQ greeter installed?",
                path.display()
            )),
            _ => HelperError::Failed(format!("couldn't open {}: {e}", path.display())),
        })?;
    let meta = f
        .metadata()
        .map_err(|e| HelperError::Failed(format!("{}: {e}", path.display())))?;
    if !meta.is_file() || meta.len() > cap {
        return Err(HelperError::Failed(format!(
            "{} isn't a regular file of at most {cap} bytes",
            path.display()
        )));
    }
    let mut s = String::new();
    f.take(cap + 1)
        .read_to_string(&mut s)
        .map_err(|e| HelperError::Failed(format!("couldn't read {}: {e}", path.display())))?;
    if s.len() as u64 > cap {
        return Err(HelperError::Failed(format!(
            "{} is larger than {cap} bytes",
            path.display()
        )));
    }
    Ok(s)
}

/// The D-Bus face of [`Helper`].
pub struct HelperIface {
    helper: Arc<Helper>,
}

fn sender_of(header: &zbus::message::Header<'_>) -> Result<String, HelperError> {
    header
        .sender()
        .map(|s| s.to_string())
        .ok_or_else(|| HelperError::NotAuthorized("the call has no sender".into()))
}

#[zbus::interface(name = "org.uq.SettingsHelper1")]
impl HelperIface {
    async fn set_logind_power_actions(
        &self,
        lid: &str,
        lid_on_ac: &str,
        power_key: &str,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), HelperError> {
        let _busy = self.helper.activity.begin();
        let sender = sender_of(&header)?;
        self.helper
            .set_logind_power_actions(&sender, lid, lid_on_ac, power_key)
            .await
    }

    async fn set_greeter_appearance(
        &self,
        theme: &str,
        variant: &str,
        hide_users: bool,
        reduce_motion: bool,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), HelperError> {
        let _busy = self.helper.activity.begin();
        let sender = sender_of(&header)?;
        self.helper
            .set_greeter_appearance(&sender, theme, variant, hide_users, reduce_motion)
            .await
    }

    async fn set_greeter_wallpaper(
        &self,
        fd: zbus::zvariant::OwnedFd,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), HelperError> {
        let _busy = self.helper.activity.begin();
        let sender = sender_of(&header)?;
        self.helper
            .set_greeter_wallpaper(&sender, OwnedFd::from(fd))
            .await
    }

    async fn clear_greeter_wallpaper(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), HelperError> {
        let _busy = self.helper.activity.begin();
        let sender = sender_of(&header)?;
        self.helper.clear_greeter_wallpaper(&sender).await
    }

    async fn set_greeter_monitors(
        &self,
        json: &str,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), HelperError> {
        let _busy = self.helper.activity.begin();
        let sender = sender_of(&header)?;
        self.helper.set_greeter_monitors(&sender, json).await
    }

    async fn set_auto_login(
        &self,
        user: &str,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), HelperError> {
        let _busy = self.helper.activity.begin();
        let sender = sender_of(&header)?;
        self.helper.set_auto_login(&sender, user).await
    }
}

/// Export the interface on `conn` and claim [`NAME`].
pub async fn serve(conn: &zbus::Connection, helper: Arc<Helper>) -> zbus::Result<()> {
    conn.object_server()
        .at(PATH, HelperIface { helper })
        .await?;
    conn.request_name(NAME).await?;
    Ok(())
}

#[cfg(test)]
mod tests;
