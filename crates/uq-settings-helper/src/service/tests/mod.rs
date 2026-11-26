use super::*;
use crate::values::HOSTILE;
use async_trait::async_trait;
use std::collections::HashMap;
use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Mutex;

/// Allows or refuses everything, and records each check.
#[derive(Default)]
struct MockAuthority {
    allow: bool,
    fail: bool,
    calls: Mutex<Vec<(String, Action)>>,
}

#[async_trait]
impl Authority for MockAuthority {
    async fn check(&self, sender: &str, action: Action) -> Result<bool, String> {
        self.calls
            .lock()
            .unwrap()
            .push((sender.to_string(), action));
        if self.fail {
            Err("polkit isn't running".into())
        } else {
            Ok(self.allow)
        }
    }
}

struct MockPasswd(HashMap<&'static str, u32>);

impl Passwd for MockPasswd {
    fn uid_of(&self, name: &str) -> io::Result<Option<u32>> {
        if name == "brokenlookup" {
            return Err(io::Error::other("nss down"));
        }
        Ok(self.0.get(name).copied())
    }

    fn shadow(&self, name: &str) -> io::Result<Option<crate::passwd::Shadow>> {
        let fine = crate::passwd::Shadow {
            locked: false,
            last_change: 19_000,
            max: -1,
            inactive: -1,
            expire: -1,
        };
        Ok(match name {
            "locked" => Some(crate::passwd::Shadow {
                locked: true,
                ..fine
            }),
            "expired" => Some(crate::passwd::Shadow { expire: 1, ..fine }),
            "brokenshadow" => return Err(io::Error::other("shadow unreadable")),
            "alice" => Some(fine),
            _ => None,
        })
    }
}

fn users() -> Arc<MockPasswd> {
    Arc::new(MockPasswd(HashMap::from([
        ("alice", 1000),
        ("bob", 60000),
        ("root", 0),
        ("daemon", 2),
        ("nobody", 65534),
        ("highuid", 60001),
        ("svc", 999),
        ("locked", 1001),
        ("expired", 1002),
        ("brokenshadow", 1003),
    ])))
}

const GREETD: &str = "[terminal]\nvt = 1\n\n[default_session]\ncommand = \"/usr/lib/uq/uq-greeter-session\"\nuser = \"greeter\"\n";

struct Fixture {
    dir: tempfile::TempDir,
    auth: Arc<MockAuthority>,
    helper: Helper,
}

impl Fixture {
    fn new(allow: bool) -> Self {
        Self::with(MockAuthority {
            allow,
            ..Default::default()
        })
    }

    fn with(auth: MockAuthority) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let rendered = dir.path().join("usr/share/uq/themes/rendered");
        for v in ["dracula-dark", "dracula-light", "forest-dark"] {
            fs::create_dir_all(rendered.join(v)).unwrap();
            fs::write(rendered.join(v).join("tokens.json"), "{}").unwrap();
        }
        // A variant directory without tokens.json is not shipped.
        fs::create_dir_all(rendered.join("forest-light")).unwrap();
        fs::create_dir_all(dir.path().join("etc/uq")).unwrap();
        fs::write(dir.path().join("etc/uq/greetd.toml"), GREETD).unwrap();
        let auth = Arc::new(auth);
        let helper = Helper::new(
            Config::with_root(dir.path()),
            auth.clone() as Arc<dyn Authority>,
            users(),
        );
        Self { dir, auth, helper }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    fn checks(&self) -> Vec<(String, Action)> {
        self.auth.calls.lock().unwrap().clone()
    }

    /// Every file under the root except the fixture's own.
    fn written(&self) -> Vec<String> {
        fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
            for e in fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, root, out);
                } else {
                    out.push(p.strip_prefix(root).unwrap().display().to_string());
                }
            }
        }
        let mut out = Vec::new();
        walk(self.dir.path(), self.dir.path(), &mut out);
        out.retain(|p| !p.starts_with("usr/share/") && p != "etc/uq/greetd.toml");
        out.sort();
        out
    }
}

fn tmpfile(bytes: &[u8]) -> File {
    let mut f = tempfile::tempfile().unwrap();
    f.write_all(bytes).unwrap();
    f
}

fn fd(f: File) -> OwnedFd {
    OwnedFd::from(f)
}

fn is_invalid(r: Result<(), HelperError>) -> bool {
    matches!(r, Err(HelperError::InvalidArgument(_)))
}

fn is_denied(r: Result<(), HelperError>) -> bool {
    matches!(r, Err(HelperError::NotAuthorized(_)))
}

mod files;
mod login;
