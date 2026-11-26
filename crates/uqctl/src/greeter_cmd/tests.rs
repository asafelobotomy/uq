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
