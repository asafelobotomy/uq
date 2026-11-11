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
mod tests {
    use super::*;
    use std::fs;

    #[derive(Default)]
    struct Fake {
        calls: Vec<String>,
        fail_on: Option<String>,
    }

    impl Runner for Fake {
        fn run(&mut self, program: &str, args: &[&str]) -> Result<(), String> {
            let call = format!("{program} {}", args.join(" "));
            self.calls.push(call.clone());
            if self.fail_on.as_deref() == Some(call.as_str()) {
                return Err(format!("{call} failed"));
            }
            Ok(())
        }
    }

    fn tree(dm: Option<&str>, greetd: bool) -> (tempfile::TempDir, Root) {
        let dir = tempfile::tempdir().unwrap();
        let root = Root(dir.path().to_path_buf());
        fs::create_dir_all(root.path("/etc/systemd/system")).unwrap();
        fs::create_dir_all(root.path("/etc/uq")).unwrap();
        fs::write(root.uq_config(), "[terminal]\nvt = 1\n").unwrap();
        if greetd {
            fs::create_dir_all(root.path("/usr/bin")).unwrap();
            fs::create_dir_all(root.path("/usr/lib/systemd/system")).unwrap();
            fs::write(root.path("/usr/bin/greetd"), "").unwrap();
            fs::write(root.path("/usr/lib/systemd/system/greetd.service"), "").unwrap();
        }
        if let Some(dm) = dm {
            set_dm(&root, dm);
        }
        (dir, root)
    }

    fn set_dm(root: &Root, unit: &str) {
        let link = root.display_manager_link();
        let _ = fs::remove_file(&link);
        std::os::unix::fs::symlink(format!("/usr/lib/systemd/system/{unit}"), link).unwrap();
    }

    #[test]
    fn refuses_without_root_and_names_run0() {
        let (_d, root) = tree(Some("sddm.service"), true);
        let mut f = Fake::default();
        let err = enable(&root, Some(1000), &mut f).unwrap_err();
        assert!(err.contains("run0 uqctl greeter enable"), "{err}");
        assert!(!err.contains("sudo"), "{err}");
        let err = disable(&root, None, &mut f).unwrap_err();
        assert!(err.contains("run0 uqctl greeter disable"), "{err}");
        assert!(f.calls.is_empty());
    }

    #[test]
    fn refuses_without_greetd() {
        let (_d, root) = tree(Some("sddm.service"), false);
        let mut f = Fake::default();
        let err = enable(&root, Some(0), &mut f).unwrap_err();
        assert!(err.contains("greetd is not installed"), "{err}");
        assert!(f.calls.is_empty());
    }

    #[test]
    fn refuses_without_uq_config() {
        let (_d, root) = tree(None, true);
        fs::remove_file(root.uq_config()).unwrap();
        let err = enable(&root, Some(0), &mut Fake::default()).unwrap_err();
        assert!(err.contains("greetd.toml is missing"), "{err}");
    }

    #[test]
    fn enable_replaces_and_records_the_current_dm() {
        let (_d, root) = tree(Some("sddm.service"), true);
        let mut f = Fake::default();
        let lines = enable(&root, Some(0), &mut f).unwrap();
        assert!(lines[0].contains("replaces sddm.service"), "{lines:?}");
        assert_eq!(
            f.calls,
            [
                "systemctl disable sddm.service",
                "systemctl enable greetd.service",
                "systemd-tmpfiles --create uq-greeter.conf"
            ]
        );
        assert_eq!(
            fs::read_to_string(root.previous_file()).unwrap(),
            "sddm.service\n"
        );
    }

    #[test]
    fn enable_without_a_dm() {
        let (_d, root) = tree(None, true);
        let mut f = Fake::default();
        let lines = enable(&root, Some(0), &mut f).unwrap();
        assert!(lines[0].contains("is on"), "{lines:?}");
        assert_eq!(f.calls[0], "systemctl enable greetd.service");
        assert!(!root.previous_file().exists());
    }

    #[test]
    fn enable_twice_changes_nothing() {
        let (_d, root) = tree(Some("greetd.service"), true);
        fs::write(root.previous_file(), "gdm.service\n").unwrap();
        let mut f = Fake::default();
        let lines = enable(&root, Some(0), &mut f).unwrap();
        assert!(lines[0].contains("already"), "{lines:?}");
        assert!(f.calls.is_empty());
        // The original record survives.
        assert_eq!(
            fs::read_to_string(root.previous_file()).unwrap(),
            "gdm.service\n"
        );
    }

    #[test]
    fn enable_failure_puts_the_old_dm_back() {
        let (_d, root) = tree(Some("sddm.service"), true);
        let mut f = Fake {
            fail_on: Some("systemctl enable greetd.service".into()),
            ..Default::default()
        };
        assert!(enable(&root, Some(0), &mut f).is_err());
        assert_eq!(f.calls.last().unwrap(), "systemctl enable sddm.service");
    }

    #[test]
    fn disable_restores_the_recorded_dm() {
        let (_d, root) = tree(Some("sddm.service"), true);
        enable(&root, Some(0), &mut Fake::default()).unwrap();
        set_dm(&root, "greetd.service");
        let mut f = Fake::default();
        let lines = disable(&root, Some(0), &mut f).unwrap();
        assert!(lines[0].contains("sddm.service"), "{lines:?}");
        assert_eq!(
            f.calls,
            [
                "systemctl disable greetd.service",
                "systemctl enable sddm.service"
            ]
        );
        assert!(!root.previous_file().exists());
    }

    #[test]
    fn disable_without_a_record() {
        let (_d, root) = tree(Some("greetd.service"), true);
        let mut f = Fake::default();
        let lines = disable(&root, Some(0), &mut f).unwrap();
        assert_eq!(f.calls, ["systemctl disable greetd.service"]);
        assert!(
            lines.join(" ").contains("No earlier display manager"),
            "{lines:?}"
        );
    }

    #[test]
    fn disable_ignores_a_bad_record() {
        let (_d, root) = tree(Some("greetd.service"), true);
        fs::write(root.previous_file(), "evil; rm -rf /\n").unwrap();
        let mut f = Fake::default();
        disable(&root, Some(0), &mut f).unwrap();
        assert_eq!(f.calls, ["systemctl disable greetd.service"]);
    }

    #[test]
    fn disable_when_greetd_is_not_the_dm() {
        let (_d, root) = tree(Some("gdm.service"), true);
        let mut f = Fake::default();
        let lines = disable(&root, Some(0), &mut f).unwrap();
        assert!(lines[0].contains("gdm.service is"), "{lines:?}");
        assert!(f.calls.is_empty());
    }

    #[test]
    fn unit_names() {
        assert!(valid_unit("sddm.service"));
        assert!(valid_unit("lightdm-gtk@x.service"));
        assert!(!valid_unit(".service"));
        assert!(!valid_unit("-x.service"));
        assert!(!valid_unit("a b.service"));
        assert!(!valid_unit("sddm"));
    }

    #[test]
    fn monitors_lua_reads_the_helper_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("greeter-monitors.toml");
        assert_eq!(monitors_lua(&file).unwrap(), "", "missing: automatic");
        fs::write(&file, "[outputs.DP-1]\nmode = \"1920x1080@60\"\n").unwrap();
        assert_eq!(
            monitors_lua(&file).unwrap(),
            "hl.monitor({ output = \"DP-1\", mode = \"1920x1080@60\", position = \"auto\", \
             scale = \"auto\" })\n"
        );
        fs::write(&file, "[outputs.DP-1]\nmode = \"1920x1080@60\"\nhack = 1\n").unwrap();
        assert!(monitors_lua(&file).unwrap_err().contains("hack"));
        fs::write(&file, vec![b'#'; uq_core::lua::MAX_GREETER_MONITORS + 10]).unwrap();
        assert!(monitors_lua(&file).is_err());
        assert!(monitors_lua(dir.path()).is_err(), "a directory");
    }
}
