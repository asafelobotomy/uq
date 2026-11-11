//! Launcher provider definitions: the `*.provider` files third-party apps install so the
//! launcher can ask them for results (spec §2.3). The bridge searches them; the Settings app
//! lists them.

use std::path::{Path, PathBuf};
use zbus::names::WellKnownName;

const MAX_NAME_CHARS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDef {
    pub bus_name: String,
    pub name: String,
    pub prefix: Option<String>,
    pub source_dir: PathBuf,
    /// `Activatable=true` (Phase 4 spec §2.4.1): the bus name has a session service file, so the
    /// launcher may start it when it opens. The bridge checks `ListActivatableNames` and ignores
    /// the key otherwise.
    pub activatable: bool,
}

/// The default definition directories: system, then the user's.
pub fn default_dirs() -> Vec<PathBuf> {
    let data = match std::env::var_os("XDG_DATA_HOME") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join(".local/share"))
            .unwrap_or_default(),
    };
    vec![
        PathBuf::from("/usr/share/uq/launcher"),
        data.join("uq/launcher"),
    ]
}

pub fn parse_def(text: &str, dir: &Path) -> Result<ProviderDef, String> {
    let (mut in_group, mut bus, mut name, mut prefix) = (false, None, None, None);
    let mut activatable = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_group = line == "[Launcher Provider]";
            continue;
        }
        if !in_group {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim().to_string();
        match k.trim() {
            "BusName" => bus = Some(v),
            "Name" => name = Some(v),
            "Prefix" => prefix = Some(v),
            "Activatable" => {
                activatable = match v.as_str() {
                    "true" => true,
                    "false" => false,
                    _ => return Err(format!("Activatable must be true or false, not {v:?}")),
                }
            }
            _ => {}
        }
    }
    let bus_name = bus.ok_or("no BusName")?;
    if bus_name.starts_with(':') || WellKnownName::try_from(bus_name.as_str()).is_err() {
        return Err(format!("invalid BusName {bus_name:?}"));
    }
    let name = name.ok_or("no Name")?;
    if name.is_empty()
        || name.chars().count() > MAX_NAME_CHARS
        || name.chars().any(char::is_control)
    {
        return Err("invalid Name".into());
    }
    if let Some(p) = &prefix
        && !((1..=4).contains(&p.chars().count())
            && p.chars().all(|c| !c.is_whitespace() && !c.is_control()))
    {
        return Err("invalid Prefix".into());
    }
    Ok(ProviderDef {
        bus_name,
        name,
        prefix,
        source_dir: dir.to_path_buf(),
        activatable,
    })
}

/// Every `*.provider` file in `dirs` with its parse result, in directory then file-name order.
/// A bus name already defined by an earlier file is an error for the later one.
pub fn scan_defs(dirs: &[PathBuf]) -> Vec<(PathBuf, Result<ProviderDef, String>)> {
    let mut out: Vec<(PathBuf, Result<ProviderDef, String>)> = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "provider"))
            .collect();
        files.sort();
        for path in files {
            let parsed = std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| parse_def(&t, dir))
                .and_then(|d| {
                    let dup = out
                        .iter()
                        .any(|(_, r)| r.as_ref().is_ok_and(|x| x.bus_name == d.bus_name));
                    if dup {
                        Err("duplicate BusName".to_string())
                    } else {
                        Ok(d)
                    }
                });
            out.push((path, parsed));
        }
    }
    out
}

/// Read every `*.provider` file in `dirs`. Invalid files are skipped with one warning each; a
/// bus name already defined by an earlier directory is not redefined by a later one.
pub fn load_defs(dirs: &[PathBuf]) -> Vec<ProviderDef> {
    scan_defs(dirs)
        .into_iter()
        .filter_map(|(path, parsed)| match parsed {
            Ok(d) => Some(d),
            Err(e) => {
                eprintln!("{}: {e}, skipped", path.display());
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_defs_skips_invalid() {
        let d = tempfile::tempdir().unwrap();
        let w = |n: &str, s: &str| std::fs::write(d.path().join(n), s).unwrap();
        w(
            "a.provider",
            "# c\n[Launcher Provider]\nBusName=org.a.B\nName=Alpha\nPrefix=:\nUnknown=1\n",
        );
        w(
            "b.provider",
            "[Launcher Provider]\nBusName=notaname\nName=Bad\n",
        );
        w(
            "c.provider",
            &format!(
                "[Launcher Provider]\nBusName=org.c.D\nName={}\n",
                "n".repeat(65)
            ),
        );
        w(
            "d.provider",
            "[Launcher Provider]\nBusName=org.d.E\nName=D\nPrefix=toolong\n",
        );
        w(
            "e.provider",
            "[Launcher Provider]\nBusName=org.e.F\nName=E\nPrefix=a b\n",
        );
        w("f.provider", "[Other]\nBusName=org.f.G\nName=F\n");
        w("g.txt", "[Launcher Provider]\nBusName=org.g.H\nName=G\n");
        w(
            "h.provider",
            "[Launcher Provider]\nBusName=org.h.I\nName=Hotel\nActivatable=true\n",
        );
        w(
            "i.provider",
            "[Launcher Provider]\nBusName=org.i.J\nName=I\nActivatable=yes\n",
        );
        w(
            "j.provider",
            "[Launcher Provider]\nBusName=org.h.I\nName=Hotel again\n",
        );
        let mut defs = load_defs(&[d.path().to_path_buf(), "/nonexistent".into()]);
        defs.sort_by(|a, b| a.bus_name.cmp(&b.bus_name));
        assert_eq!(defs.len(), 2, "{defs:?}");
        assert_eq!(defs[0].bus_name, "org.a.B");
        assert_eq!(defs[0].prefix.as_deref(), Some(":"));
        assert_eq!(defs[0].source_dir, d.path());
        assert!(!defs[0].activatable);
        assert_eq!(defs[1].name, "Hotel");
        assert_eq!(defs[1].prefix, None);
        assert!(defs[1].activatable);

        let scanned = scan_defs(&[d.path().to_path_buf()]);
        let err = |f: &str| {
            scanned
                .iter()
                .find(|(p, _)| p.ends_with(f))
                .and_then(|(_, r)| r.as_ref().err().cloned())
        };
        assert_eq!(err("j.provider").as_deref(), Some("duplicate BusName"));
        assert!(err("i.provider").unwrap().contains("Activatable"));
        assert_eq!(err("a.provider"), None);
        assert_eq!(scanned.len(), 9, "g.txt is not a provider");
    }
}
