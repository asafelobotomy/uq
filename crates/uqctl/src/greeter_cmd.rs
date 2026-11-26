//! `uqctl greeter enable|disable` (spec phase 5 §2.6): make greetd, with uQ's config and
//! greeter, the display manager, or switch back to the one it replaced.
//!
//! Root only, through `run0 uqctl greeter enable` (never sudo). `enable` refuses when greetd isn't
//! installed, names the display manager it replaces and records it in
//! `/etc/uq/greeter-previous-dm`; `disable` re-enables that one. The filesystem root and the
//! commands are injectable, so the tests run against a temporary tree and a fake `systemctl`.

use std::path::PathBuf;
use std::process::ExitCode;

const GREETD: &str = "greetd.service";

#[derive(clap::Subcommand, Debug, PartialEq, Eq)]
pub enum GreeterCmd {
    /// Make greetd with uQ's login screen the display manager (as root: `run0 uqctl greeter enable`).
    Enable,
    /// Switch back to the display manager uQ's login screen replaced.
    Disable,
    /// For uq-greeter-session: print the `hl.monitor` lines for the login screen from
    /// /etc/uq/greeter-monitors.toml. Prints nothing (automatic layout) when the file is missing;
    /// exits 1, printing nothing, when it is unreadable or doesn't pass uqd's monitor checks.
    #[command(hide = true)]
    MonitorsLua {
        #[arg(long, value_name = "FILE", default_value = GREETER_MONITORS)]
        file: PathBuf,
    },
    /// Match the login screen's theme to the given theme (uqd runs this after each theme change;
    /// an active local administrator needs no password). Fails quietly-but-nonzero for a theme
    /// the login screen doesn't ship, such as your own.
    #[command(hide = true)]
    SyncTheme {
        #[arg(long)]
        theme: String,
        #[arg(long)]
        variant: String,
    },
}

/// Written by uq-settings-helper's `SetGreeterMonitors`.
pub const GREETER_MONITORS: &str = "/etc/uq/greeter-monitors.toml";

/// `monitors-lua`: the lines to print, or why the file was refused. A missing file is the
/// automatic layout: no lines.
pub fn monitors_lua(file: &std::path::Path) -> Result<String, String> {
    use std::io::Read;
    let f = match std::fs::File::open(file) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(e) => return Err(format!("{}: {e}", file.display())),
    };
    if !f.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err(format!("{}: not a regular file", file.display()));
    }
    let mut text = String::new();
    f.take(uq_core::lua::MAX_GREETER_MONITORS as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|e| format!("{}: {e}", file.display()))?;
    uq_core::lua::greeter_monitors_lua(&text).map_err(|e| format!("{}: {e}", file.display()))
}

/// Runs a system command (`systemctl …`, `systemd-tmpfiles …`); Err carries its message.
pub trait Runner {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<(), String>;
}

pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<(), String> {
        let out = std::process::Command::new(program)
            .args(args)
            .output()
            .map_err(|e| format!("running {program}: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "`{program} {}` failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }
}

/// Where the files live: `/` normally, a temporary directory in tests.
pub struct Root(pub PathBuf);

impl Root {
    fn path(&self, abs: &str) -> PathBuf {
        self.0.join(abs.trim_start_matches('/'))
    }
    fn display_manager_link(&self) -> PathBuf {
        self.path("/etc/systemd/system/display-manager.service")
    }
    fn previous_file(&self) -> PathBuf {
        self.path("/etc/uq/greeter-previous-dm")
    }
    fn greetd_installed(&self) -> bool {
        self.path("/usr/bin/greetd").is_file()
            && self
                .path("/usr/lib/systemd/system/greetd.service")
                .is_file()
    }
    fn uq_config(&self) -> PathBuf {
        self.path("/etc/uq/greetd.toml")
    }
}

/// A unit name we are willing to pass to systemctl (`sddm.service`, `gdm.service`, …).
fn valid_unit(name: &str) -> bool {
    name.len() <= 128
        && name.ends_with(".service")
        && name.len() > ".service".len()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "@._-".contains(c))
}

/// The unit `display-manager.service` points at, if any.
fn current_dm(root: &Root) -> Option<String> {
    let target = std::fs::read_link(root.display_manager_link()).ok()?;
    let name = target.file_name()?.to_str()?.to_string();
    valid_unit(&name).then_some(name)
}

fn previous_dm(root: &Root) -> Option<String> {
    let text = std::fs::read_to_string(root.previous_file()).ok()?;
    let name = text.trim().to_string();
    (valid_unit(&name) && name != GREETD).then_some(name)
}

fn need_root(uid: Option<u32>, verb: &str) -> Result<(), String> {
    if uid == Some(0) {
        Ok(())
    } else {
        Err(format!(
            "this changes the system's display manager, so it needs root: run `run0 uqctl greeter {verb}`"
        ))
    }
}

/// Returns the lines to print.
pub fn enable(root: &Root, uid: Option<u32>, run: &mut dyn Runner) -> Result<Vec<String>, String> {
    need_root(uid, "enable")?;
    if !root.greetd_installed() {
        return Err(
            "greetd is not installed: install it (`run0 pacman -S greetd`), then run `run0 uqctl greeter enable` again"
                .into(),
        );
    }
    if !root.uq_config().is_file() {
        return Err(format!(
            "{} is missing: reinstall the uq package to restore it",
            root.uq_config().display()
        ));
    }
    let current = current_dm(root);
    let mut lines = Vec::new();
    if current.as_deref() == Some(GREETD) {
        lines.push("greetd is already the display manager; it uses uQ's login screen.".into());
        return Ok(lines);
    }
    if let Some(cur) = &current {
        write_previous(root, cur)?;
        run.run("systemctl", &["disable", cur])?;
    }
    if let Err(e) = run.run("systemctl", &["enable", GREETD]) {
        // Put the old display manager back rather than leave the machine without one.
        if let Some(cur) = &current {
            let _ = run.run("systemctl", &["enable", cur]);
        }
        return Err(e);
    }
    // The greeter's state directory is owned by `greeter`, which exists now that greetd does.
    let _ = run.run("systemd-tmpfiles", &["--create", "uq-greeter.conf"]);
    lines.push(match &current {
        Some(cur) => {
            format!("uQ's login screen replaces {cur} (greetd is now the display manager).")
        }
        None => "uQ's login screen is on (greetd is now the display manager).".into(),
    });
    lines.push("It appears at the next boot. To switch back: `run0 uqctl greeter disable`.".into());
    Ok(lines)
}

fn write_previous(root: &Root, unit: &str) -> Result<(), String> {
    let path = root.previous_file();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    uq_core::atomic::atomic_write(&path, &format!("{unit}\n"))
        .map_err(|e| format!("{}: {e}", path.display()))
}

pub fn disable(root: &Root, uid: Option<u32>, run: &mut dyn Runner) -> Result<Vec<String>, String> {
    need_root(uid, "disable")?;
    let current = current_dm(root);
    if current.as_deref() != Some(GREETD) {
        return Ok(vec![match current {
            Some(cur) => {
                format!("greetd is not the display manager ({cur} is); nothing to change.")
            }
            None => "greetd is not the display manager; nothing to change.".into(),
        }]);
    }
    let previous = previous_dm(root);
    run.run("systemctl", &["disable", GREETD])?;
    let mut lines = Vec::new();
    match previous {
        Some(prev) => {
            if let Err(e) = run.run("systemctl", &["enable", &prev]) {
                let _ = run.run("systemctl", &["enable", GREETD]);
                return Err(e);
            }
            let _ = std::fs::remove_file(root.previous_file());
            lines.push(format!(
                "Switched back to {prev}; it appears at the next boot."
            ));
        }
        None => {
            lines.push("greetd is off. No earlier display manager was recorded:".into());
            lines.push(
                "enable one with `run0 systemctl enable <name>.service`, or you get a text login."
                    .into(),
            );
        }
    }
    Ok(lines)
}

pub fn main(cmd: &GreeterCmd) -> ExitCode {
    let root = Root(PathBuf::from("/"));
    let uid = crate::setup::current_uid();
    let res = match cmd {
        GreeterCmd::Enable => enable(&root, uid, &mut SystemRunner),
        GreeterCmd::Disable => disable(&root, uid, &mut SystemRunner),
        GreeterCmd::SyncTheme { .. } => unreachable!("handled by sync_theme in main"),
        GreeterCmd::MonitorsLua { file } => {
            return match monitors_lua(file) {
                Ok(lines) => {
                    print!("{lines}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("uqctl: the login screen's monitor layout was not used: {e}");
                    ExitCode::FAILURE
                }
            };
        }
    };
    match res {
        Ok(lines) => {
            lines.iter().for_each(|l| println!("{l}"));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("uqctl: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests;

#[zbus::proxy(
    interface = "org.uq.SettingsHelper1",
    default_service = "org.uq.SettingsHelper1",
    default_path = "/org/uq/SettingsHelper1"
)]
trait Helper {
    fn sync_greeter_theme(&self, theme: &str, variant: &str) -> zbus::Result<()>;
}

/// `sync-theme`: ask uq-settings-helper (system bus, polkit-checked) to write the theme.
pub async fn sync_theme(theme: &str, variant: &str) -> ExitCode {
    let res: zbus::Result<()> = async {
        let conn = zbus::Connection::system().await?;
        HelperProxy::new(&conn)
            .await?
            .sync_greeter_theme(theme, variant)
            .await
    }
    .await;
    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("uqctl: the login screen's theme was not changed: {e}");
            ExitCode::FAILURE
        }
    }
}
