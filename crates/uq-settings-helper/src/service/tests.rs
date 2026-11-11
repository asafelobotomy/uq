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

#[tokio::test]
async fn logind_writes_the_template_after_polkit_power() {
    let f = Fixture::new(true);
    f.helper
        .set_logind_power_actions(":1.5", "hibernate", "ignore", "lock")
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(f.path("etc/systemd/logind.conf.d/50-uq.conf")).unwrap(),
        templates::logind(
            PowerAction::Hibernate,
            PowerAction::Ignore,
            PowerAction::Lock
        )
    );
    assert_eq!(f.checks(), [(":1.5".to_string(), Action::Power)]);
}

#[tokio::test]
async fn logind_rejects_anything_outside_the_set_before_asking_polkit() {
    let f = Fixture::new(true);
    for bad in HOSTILE
        .iter()
        .chain(&["reboot", "Suspend", "suspend\n[Login]"])
    {
        for args in [
            (*bad, "suspend", "suspend"),
            ("suspend", *bad, "suspend"),
            ("suspend", "suspend", *bad),
        ] {
            let r = f
                .helper
                .set_logind_power_actions(":1.5", args.0, args.1, args.2)
                .await;
            assert!(is_invalid(r), "{args:?}");
        }
    }
    assert!(f.checks().is_empty());
    assert!(f.written().is_empty());
}

#[tokio::test]
async fn greeter_appearance_needs_a_shipped_variant() {
    let f = Fixture::new(true);
    f.helper
        .set_greeter_appearance(":1.5", "dracula", "light", true, true)
        .await
        .unwrap();
    let t: toml::Table = fs::read_to_string(f.path("etc/uq/greeter.toml"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(t["theme"].as_str(), Some("dracula"));
    assert_eq!(t["variant"].as_str(), Some("light"));
    assert_eq!(f.checks(), [(":1.5".to_string(), Action::Greeter)]);

    // Valid names that aren't shipped, or are shipped without tokens.json.
    for (theme, variant) in [
        ("nope", "dark"),
        ("forest", "light"),
        ("dracula-dark", "dark"),
    ] {
        let r = f
            .helper
            .set_greeter_appearance(":1.5", theme, variant, false, false)
            .await;
        assert!(is_invalid(r), "{theme}-{variant}");
    }
    for bad in HOSTILE.iter().chain(&["../rendered/dracula", "Dracula"]) {
        assert!(is_invalid(
            f.helper
                .set_greeter_appearance(":1.5", bad, "dark", false, false)
                .await
        ));
        assert!(is_invalid(
            f.helper
                .set_greeter_appearance(":1.5", "dracula", bad, false, false)
                .await
        ));
    }
    assert_eq!(f.checks().len(), 1);
}

#[tokio::test]
async fn greeter_appearance_ignores_a_tokens_symlink_to_nothing() {
    let f = Fixture::new(true);
    let d = f.path("usr/share/uq/themes/rendered/ghost-dark");
    fs::create_dir_all(&d).unwrap();
    std::os::unix::fs::symlink("/nonexistent/x", d.join("tokens.json")).unwrap();
    assert!(is_invalid(
        f.helper
            .set_greeter_appearance(":1.5", "ghost", "dark", false, false)
            .await
    ));
}

#[tokio::test]
async fn wallpaper_is_copied_byte_for_byte_from_offset_zero() {
    let f = Fixture::new(true);
    // Not an image at all: it is never decoded, only copied.
    let bytes: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 256) as u8).collect();
    let mut file = tmpfile(&bytes);
    // The caller's offset is left wherever it was, and doesn't change what is copied.
    file.seek(SeekFrom::Start(1234)).unwrap();
    f.helper
        .set_greeter_wallpaper(":1.5", fd(file))
        .await
        .unwrap();
    let out = f.path("var/lib/uq-greeter/wallpaper");
    assert_eq!(fs::read(&out).unwrap(), bytes);
    use std::os::unix::fs::MetadataExt;
    assert_eq!(fs::metadata(&out).unwrap().mode() & 0o7777, 0o644);
    assert_eq!(f.checks(), [(":1.5".to_string(), Action::Greeter)]);
}

#[tokio::test]
async fn wallpaper_at_exactly_the_cap_is_accepted_and_one_byte_more_is_not() {
    let f = Fixture::new(true);
    let file = tempfile::tempfile().unwrap();
    file.set_len(MAX_WALLPAPER).unwrap();
    f.helper
        .set_greeter_wallpaper(":1.5", fd(file))
        .await
        .unwrap();
    assert_eq!(
        fs::metadata(f.path("var/lib/uq-greeter/wallpaper"))
            .unwrap()
            .len(),
        MAX_WALLPAPER
    );

    let f = Fixture::new(true);
    let file = tempfile::tempfile().unwrap();
    file.set_len(MAX_WALLPAPER + 1).unwrap();
    assert!(is_invalid(
        f.helper.set_greeter_wallpaper(":1.5", fd(file)).await
    ));
    assert!(f.checks().is_empty());
    assert!(f.written().is_empty());
}

#[test]
fn read_capped_refuses_a_file_that_grew_past_the_cap() {
    let file = tmpfile(&[1u8; 100]);
    assert_eq!(read_capped(&file, 100).unwrap().len(), 100);
    assert!(read_capped(&file, 99).is_err());
}

#[tokio::test]
async fn wallpaper_refuses_directories_fifos_sockets_and_devices() {
    let f = Fixture::new(true);
    let scratch = tempfile::tempdir().unwrap();

    // A directory fd.
    let dir = File::open(scratch.path()).unwrap();
    assert!(is_invalid(
        f.helper.set_greeter_wallpaper(":1.5", fd(dir)).await
    ));

    // A FIFO, opened without blocking: refused by fstat, never read (a read would hang).
    let fifo = scratch.path().join("fifo");
    let c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    let fifo_fd = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&fifo)
        .unwrap();
    assert!(is_invalid(
        f.helper.set_greeter_wallpaper(":1.5", fd(fifo_fd)).await
    ));

    // A pipe and a socket.
    let (r, _w) = std::io::pipe().unwrap();
    assert!(is_invalid(
        f.helper
            .set_greeter_wallpaper(":1.5", OwnedFd::from(r))
            .await
    ));
    let (s, _t) = std::os::unix::net::UnixStream::pair().unwrap();
    assert!(is_invalid(
        f.helper
            .set_greeter_wallpaper(":1.5", OwnedFd::from(s))
            .await
    ));

    // Character devices.
    for dev in ["/dev/null", "/dev/zero", "/dev/urandom"] {
        let d = File::open(dev).unwrap();
        assert!(is_invalid(
            f.helper.set_greeter_wallpaper(":1.5", fd(d)).await
        ));
    }

    // A file opened through a symlink is whatever the caller opened: still a regular file,
    // and still only readable with the caller's own rights. It is copied like any other.
    let real = scratch.path().join("real");
    fs::write(&real, b"img").unwrap();
    std::os::unix::fs::symlink(&real, scratch.path().join("link")).unwrap();
    let via_link = File::open(scratch.path().join("link")).unwrap();
    f.helper
        .set_greeter_wallpaper(":1.5", fd(via_link))
        .await
        .unwrap();
    assert_eq!(
        fs::read(f.path("var/lib/uq-greeter/wallpaper")).unwrap(),
        b"img"
    );
    assert_eq!(f.checks().len(), 1, "only the regular file reached polkit");
}

#[tokio::test]
async fn wallpaper_refuses_an_unreadable_fd() {
    let f = Fixture::new(true);
    let scratch = tempfile::tempdir().unwrap();
    let p = scratch.path().join("w");
    fs::write(&p, b"data").unwrap();
    let write_only = OpenOptions::new().write(true).open(&p).unwrap();
    assert!(is_invalid(
        f.helper.set_greeter_wallpaper(":1.5", fd(write_only)).await
    ));
    assert!(f.written().is_empty());
}

#[tokio::test]
async fn clear_wallpaper_removes_it_and_is_idempotent() {
    let f = Fixture::new(true);
    f.helper
        .set_greeter_wallpaper(":1.5", fd(tmpfile(b"x")))
        .await
        .unwrap();
    f.helper.clear_greeter_wallpaper(":1.5").await.unwrap();
    assert!(!f.path("var/lib/uq-greeter/wallpaper").exists());
    f.helper.clear_greeter_wallpaper(":1.5").await.unwrap();
    assert_eq!(
        f.checks().iter().map(|c| c.1).collect::<Vec<_>>(),
        [Action::Greeter; 3]
    );
}

#[tokio::test]
async fn clear_monitors_removes_the_file_and_is_idempotent() {
    let f = Fixture::new(true);
    f.helper
        .set_greeter_monitors(":1.5", r#"{"outputs":{"DP-1":{"mode":"1920x1080@60"}}}"#)
        .await
        .unwrap();
    assert!(f.path("etc/uq/greeter-monitors.toml").exists());
    f.helper.clear_greeter_monitors(":1.5").await.unwrap();
    assert!(!f.path("etc/uq/greeter-monitors.toml").exists());
    f.helper.clear_greeter_monitors(":1.5").await.unwrap();
    assert_eq!(
        f.checks().iter().map(|c| c.1).collect::<Vec<_>>(),
        [Action::Greeter; 3]
    );
}

#[tokio::test]
async fn monitors_go_through_uq_core_validation() {
    let f = Fixture::new(true);
    f.helper
        .set_greeter_monitors(
            ":1.5",
            r#"{"outputs":{"DP-1":{"mode":"1920x1080@60","scale":1.5,"transform":1}}}"#,
        )
        .await
        .unwrap();
    let back: Monitors =
        toml::from_str(&fs::read_to_string(f.path("etc/uq/greeter-monitors.toml")).unwrap())
            .unwrap();
    assert_eq!(back.outputs["DP-1"].mode.as_deref(), Some("1920x1080@60"));
    assert_eq!(back.outputs["DP-1"].transform, Some(1));

    let huge = format!(
        r#"{{"outputs":{{"DP-1":{{"description":"{}"}}}}}}"#,
        "a".repeat(70_000)
    );
    for bad in [
        "",
        "null",
        "[]",
        "not json",
        r#"{"outputs":{"../etc":{}}}"#,
        r#"{"outputs":{"DP 1":{}}}"#,
        r#"{"outputs":{"DP-1\n[x]":{}}}"#,
        r#"{"outputs":{"DP-1":{"mode":"1920x1080\"\nevil=1"}}}"#,
        r#"{"outputs":{"DP-1":{"scale":99}}}"#,
        r#"{"outputs":{"DP-1":{"transform":8}}}"#,
        r#"{"outputs":{"DP-1":{"mirror_of":"DP-1"}}}"#,
        r#"{"outputs":{"DP-1":{"unknown":1}}}"#,
        r#"{"outputs":{},"extra":1}"#,
        r#"{"outputs":{"DP-1":{"scale":"1"}}}"#,
        huge.as_str(),
    ] {
        assert!(
            is_invalid(f.helper.set_greeter_monitors(":1.5", bad).await),
            "{}",
            &bad[..bad.len().min(60)]
        );
    }
    assert_eq!(f.checks().len(), 1);
}

#[tokio::test]
async fn auto_login_sets_and_removes_initial_session_only() {
    let f = Fixture::new(true);
    f.helper.set_auto_login(":1.5", "alice").await.unwrap();
    let text = fs::read_to_string(f.path("etc/uq/greetd.toml")).unwrap();
    assert!(text.starts_with(GREETD));
    let t: toml::Table = text.parse().unwrap();
    assert_eq!(t["initial_session"]["user"].as_str(), Some("alice"));
    assert_eq!(
        t["initial_session"]["command"].as_str(),
        Some("/usr/bin/uq-session")
    );
    f.helper.set_auto_login(":1.5", "bob").await.unwrap();
    let t: toml::Table = fs::read_to_string(f.path("etc/uq/greetd.toml"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(t["initial_session"]["user"].as_str(), Some("bob"));
    f.helper.set_auto_login(":1.5", "").await.unwrap();
    assert_eq!(
        fs::read_to_string(f.path("etc/uq/greetd.toml")).unwrap(),
        GREETD
    );
    assert_eq!(
        f.checks().iter().map(|c| c.1).collect::<Vec<_>>(),
        [Action::AutoLogin; 3]
    );
}

#[tokio::test]
async fn auto_login_refuses_system_missing_and_malformed_users() {
    let f = Fixture::new(true);
    for bad in ["root", "daemon", "nobody", "highuid", "svc", "ghost"] {
        assert!(
            is_invalid(f.helper.set_auto_login(":1.5", bad).await),
            "{bad}"
        );
    }
    for bad in HOSTILE.iter().filter(|s| !s.is_empty()).chain(&[
        "Alice",
        "alice\n",
        "alice\nuser = \"root\"",
        "alice\"",
        "1alice",
    ]) {
        assert!(
            is_invalid(f.helper.set_auto_login(":1.5", bad).await),
            "{bad:?}"
        );
    }
    assert!(matches!(
        f.helper.set_auto_login(":1.5", "brokenlookup").await,
        Err(HelperError::Failed(_))
    ));
    assert!(f.checks().is_empty());
    assert_eq!(
        fs::read_to_string(f.path("etc/uq/greetd.toml")).unwrap(),
        GREETD
    );
}

#[tokio::test]
async fn auto_login_refuses_locked_and_expired_accounts() {
    let f = Fixture::new(true);
    for bad in ["locked", "expired"] {
        let r = f.helper.set_auto_login(":1.5", bad).await;
        assert!(is_invalid(r), "{bad}");
    }
    assert!(matches!(
        f.helper.set_auto_login(":1.5", "brokenshadow").await,
        Err(HelperError::Failed(_))
    ));
    assert!(f.checks().is_empty(), "refused before polkit is asked");
    assert_eq!(
        fs::read_to_string(f.path("etc/uq/greetd.toml")).unwrap(),
        GREETD
    );
    // An account with a usable shadow entry, and one without any (bob), are fine.
    f.helper.set_auto_login(":1.5", "alice").await.unwrap();
    f.helper.set_auto_login(":1.5", "bob").await.unwrap();
}

#[tokio::test]
async fn auto_login_needs_an_existing_valid_greetd_toml() {
    let f = Fixture::new(true);
    fs::remove_file(f.path("etc/uq/greetd.toml")).unwrap();
    assert!(matches!(
        f.helper.set_auto_login(":1.5", "alice").await,
        Err(HelperError::Failed(_))
    ));
    assert!(!f.path("etc/uq/greetd.toml").exists());

    fs::write(f.path("etc/uq/greetd.toml"), "[default_session\n").unwrap();
    assert!(matches!(
        f.helper.set_auto_login(":1.5", "alice").await,
        Err(HelperError::Failed(_))
    ));
    assert_eq!(
        fs::read_to_string(f.path("etc/uq/greetd.toml")).unwrap(),
        "[default_session\n"
    );

    // A symlink there is not followed.
    fs::remove_file(f.path("etc/uq/greetd.toml")).unwrap();
    let elsewhere = f.path("elsewhere.toml");
    fs::write(&elsewhere, GREETD).unwrap();
    std::os::unix::fs::symlink(&elsewhere, f.path("etc/uq/greetd.toml")).unwrap();
    assert!(matches!(
        f.helper.set_auto_login(":1.5", "alice").await,
        Err(HelperError::Failed(_))
    ));
    assert_eq!(fs::read_to_string(&elsewhere).unwrap(), GREETD);

    // Oversized.
    fs::remove_file(f.path("etc/uq/greetd.toml")).unwrap();
    let big = format!("{GREETD}# {}\n", "x".repeat(MAX_GREETD_TOML as usize));
    fs::write(f.path("etc/uq/greetd.toml"), &big).unwrap();
    assert!(matches!(
        f.helper.set_auto_login(":1.5", "alice").await,
        Err(HelperError::Failed(_))
    ));
}

/// Every method, with valid arguments, against an authority that refuses (or can't be
/// reached): nothing is written, and each asked for the right action.
#[tokio::test]
async fn an_unauthorised_caller_writes_nothing() {
    for auth in [
        MockAuthority::default(),
        MockAuthority {
            fail: true,
            ..Default::default()
        },
    ] {
        let f = Fixture::with(auth);
        // Pre-existing files must survive untouched too.
        fs::create_dir_all(f.path("var/lib/uq-greeter")).unwrap();
        fs::write(f.path("var/lib/uq-greeter/wallpaper"), b"old").unwrap();
        let h = &f.helper;
        assert!(is_denied(
            h.set_logind_power_actions(":1.9", "suspend", "suspend", "poweroff")
                .await
        ));
        assert!(is_denied(
            h.set_greeter_appearance(":1.9", "dracula", "dark", false, false)
                .await
        ));
        assert!(is_denied(
            h.set_greeter_wallpaper(":1.9", fd(tmpfile(b"new"))).await
        ));
        assert!(is_denied(h.clear_greeter_wallpaper(":1.9").await));
        assert!(is_denied(
            h.set_greeter_monitors(":1.9", r#"{"outputs":{}}"#).await
        ));
        assert!(is_denied(h.clear_greeter_monitors(":1.9").await));
        assert!(is_denied(h.set_auto_login(":1.9", "alice").await));
        assert!(is_denied(h.set_auto_login(":1.9", "").await));
        assert_eq!(f.written(), ["var/lib/uq-greeter/wallpaper"]);
        assert_eq!(
            fs::read(f.path("var/lib/uq-greeter/wallpaper")).unwrap(),
            b"old"
        );
        assert_eq!(
            fs::read_to_string(f.path("etc/uq/greetd.toml")).unwrap(),
            GREETD
        );
        let actions: Vec<Action> = f.checks().iter().map(|c| c.1).collect();
        assert_eq!(
            actions,
            [
                Action::Power,
                Action::Greeter,
                Action::Greeter,
                Action::Greeter,
                Action::Greeter,
                Action::Greeter,
                Action::AutoLogin,
                Action::AutoLogin
            ]
        );
        assert!(f.checks().iter().all(|c| c.0 == ":1.9"));
    }
}

#[tokio::test]
async fn nothing_lands_outside_the_fixed_paths() {
    let f = Fixture::new(true);
    let h = &f.helper;
    h.set_logind_power_actions(":1.5", "lock", "lock", "lock")
        .await
        .unwrap();
    h.set_greeter_appearance(":1.5", "forest", "dark", false, false)
        .await
        .unwrap();
    h.set_greeter_wallpaper(":1.5", fd(tmpfile(b"w")))
        .await
        .unwrap();
    h.set_greeter_monitors(":1.5", "{}").await.unwrap();
    h.set_auto_login(":1.5", "alice").await.unwrap();
    assert_eq!(
        f.written(),
        [
            "etc/systemd/logind.conf.d/50-uq.conf",
            "etc/uq/greeter-monitors.toml",
            "etc/uq/greeter.toml",
            "var/lib/uq-greeter/wallpaper",
        ]
    );
}
