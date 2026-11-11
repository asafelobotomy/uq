//! The real `uq-settings-helper` service on the private bus, with a mock polkit authority and
//! every file under a temporary root, so the pages are tested against the helper's own checks.

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use uq_settings_helper::passwd::Passwd;
use uq_settings_helper::{Action, Authority, Config, Helper};

use super::{Deny, SysRig};

struct MockAuthority(Deny);

#[async_trait]
impl Authority for MockAuthority {
    async fn check(&self, _sender: &str, _action: Action) -> Result<bool, String> {
        Ok(!self.0.on())
    }
}

struct MockPasswd(HashMap<String, u32>);

impl Passwd for MockPasswd {
    fn uid_of(&self, name: &str) -> io::Result<Option<u32>> {
        Ok(self.0.get(name).copied())
    }
}

/// The greetd config the package ships, before any auto-login.
pub const GREETD: &str = "[terminal]\nvt = 1\n\n[default_session]\ncommand = \"/usr/lib/uq/uq-greeter-session\"\nuser = \"greeter\"\n";

pub struct FakeHelper {
    pub root: tempfile::TempDir,
    pub deny: Deny,
}

impl FakeHelper {
    /// The helper's files only (the shipped greetd config and the pre-rendered graphite dark
    /// and light and dracula dark variants), with no service on the bus: a missing helper.
    pub fn files_only() -> Self {
        let root = tempfile::tempdir().unwrap();
        let rendered = root.path().join("usr/share/uq/themes/rendered");
        for v in ["graphite-dark", "graphite-light", "dracula-dark"] {
            std::fs::create_dir_all(rendered.join(v)).unwrap();
            std::fs::write(rendered.join(v).join("tokens.json"), "{}").unwrap();
        }
        std::fs::create_dir_all(root.path().join("etc/uq")).unwrap();
        std::fs::write(root.path().join("etc/uq/greetd.toml"), GREETD).unwrap();
        Self {
            root,
            deny: Deny::default(),
        }
    }

    /// [`Self::files_only`], and the helper serving them, with `users` (name, uid) known to it.
    pub async fn start(rig: &mut SysRig, users: &[(&str, u32)]) -> Self {
        let me = Self::files_only();
        let passwd = MockPasswd(users.iter().map(|(n, u)| ((*n).to_string(), *u)).collect());
        let helper = Helper::new(
            Config::with_root(me.root.path()),
            Arc::new(MockAuthority(me.deny.clone())),
            Arc::new(passwd),
        );
        let c = uq_core::test_bus::connect(&rig.bus).await;
        uq_settings_helper::serve(&c, Arc::new(helper))
            .await
            .unwrap();
        rig.keep(c);
        me
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.path().join(rel)
    }

    pub fn read(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.path(rel)).ok()
    }
}
