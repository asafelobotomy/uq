//! What logind is configured to do for the lid and the power key: `logind.conf` and its
//! drop-ins, read the way systemd reads them, under a root prefix (`/`, or a test directory).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The actions uQ offers (the helper accepts exactly these).
pub const CHOICES: [&str; 5] = ["suspend", "hibernate", "poweroff", "lock", "ignore"];

/// The three settings, by logind key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actions {
    pub lid: String,
    pub lid_on_ac: String,
    pub power_key: String,
}

impl Default for Actions {
    /// logind's built-in defaults.
    fn default() -> Self {
        Self {
            lid: "suspend".into(),
            lid_on_ac: "suspend".into(),
            power_key: "poweroff".into(),
        }
    }
}

/// The main file, then each drop-in by file name (an `/etc` one hides a same-named one in
/// `/run` or `/usr/lib`).
pub fn config_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let main = root.join("etc/systemd/logind.conf");
    let vendor = root.join("usr/lib/systemd/logind.conf");
    if main.is_file() {
        out.push(main);
    } else if vendor.is_file() {
        out.push(vendor);
    }
    let mut dropins: BTreeMap<String, PathBuf> = BTreeMap::new();
    for dir in [
        "usr/lib/systemd/logind.conf.d",
        "run/systemd/logind.conf.d",
        "etc/systemd/logind.conf.d",
    ] {
        let Ok(rd) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.ends_with(".conf") && e.path().is_file() {
                dropins.insert(name, e.path());
            }
        }
    }
    out.extend(dropins.into_values());
    out
}

/// Apply one file's `[Login]` keys to `a`. `lid_on_ac_set` records whether
/// `HandleLidSwitchExternalPower` was given anywhere (unset, it follows `HandleLidSwitch`).
fn apply(text: &str, a: &mut Actions, lid_on_ac_set: &mut bool) {
    let mut in_login = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_login = line == "[Login]";
            continue;
        }
        if !in_login || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim().to_string();
        match k.trim() {
            "HandleLidSwitch" => a.lid = v,
            "HandleLidSwitchExternalPower" => {
                a.lid_on_ac = v;
                *lid_on_ac_set = true;
            }
            "HandlePowerKey" => a.power_key = v,
            _ => {}
        }
    }
}

/// The configured actions under `root`.
pub fn configured(root: &Path) -> Actions {
    let mut a = Actions::default();
    let mut lid_on_ac_set = false;
    for f in config_files(root) {
        if let Ok(text) = std::fs::read_to_string(&f) {
            apply(&text, &mut a, &mut lid_on_ac_set);
        }
    }
    if !lid_on_ac_set || a.lid_on_ac.is_empty() {
        a.lid_on_ac = a.lid.clone();
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn defaults_without_files() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(configured(d.path()), Actions::default());
    }

    #[test]
    fn dropins_override_in_name_order_and_etc_hides_usr() {
        let d = tempfile::tempdir().unwrap();
        write(
            d.path(),
            "etc/systemd/logind.conf",
            "[Login]\n#HandleLidSwitch=ignore\nHandlePowerKey=suspend\n",
        );
        write(
            d.path(),
            "usr/lib/systemd/logind.conf.d/10-vendor.conf",
            "[Login]\nHandleLidSwitch=hibernate\n",
        );
        write(
            d.path(),
            "usr/lib/systemd/logind.conf.d/50-uq.conf",
            "[Login]\nHandleLidSwitch=ignore\n",
        );
        write(
            d.path(),
            "etc/systemd/logind.conf.d/50-uq.conf",
            "[Login]\nHandleLidSwitch=lock\nHandleLidSwitchExternalPower=ignore\n",
        );
        write(
            d.path(),
            "etc/systemd/logind.conf.d/notes.txt",
            "[Login]\nHandlePowerKey=ignore\n",
        );
        let a = configured(d.path());
        assert_eq!(a.lid, "lock");
        assert_eq!(a.lid_on_ac, "ignore");
        assert_eq!(a.power_key, "suspend");
    }

    #[test]
    fn lid_on_ac_follows_the_lid_when_unset() {
        let d = tempfile::tempdir().unwrap();
        write(
            d.path(),
            "usr/lib/systemd/logind.conf",
            "[Login]\nHandleLidSwitch=hibernate\n[Other]\nHandlePowerKey=ignore\n",
        );
        let a = configured(d.path());
        assert_eq!(
            (a.lid.as_str(), a.lid_on_ac.as_str()),
            ("hibernate", "hibernate")
        );
        assert_eq!(a.power_key, "poweroff", "keys outside [Login] are ignored");
    }
}
