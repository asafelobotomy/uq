//! The helper on a private bus, with a fake polkit Authority object: the real `Polkit` client,
//! the D-Bus interface, fd passing and the error names.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::sync::{Arc, Mutex};
use uq_core::test_bus::{Bus, connect, private_bus};
use uq_settings_helper::passwd::Passwd;
use uq_settings_helper::{Config, Helper, IFACE, NAME, PATH, Polkit, serve};
use zbus::zvariant::OwnedValue;

#[derive(Clone, Debug, PartialEq)]
struct Check {
    kind: String,
    name: String,
    action: String,
    flags: u32,
}

/// What the fake authority answers.
#[derive(Clone, Copy, PartialEq)]
enum Answer {
    Allow,
    Deny,
    Cancel,
}

struct FakePolkit {
    answer: Arc<Mutex<Answer>>,
    checks: Arc<Mutex<Vec<Check>>>,
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.PolicyKit1.Error")]
enum PolkitError {
    #[zbus(error)]
    ZBus(zbus::Error),
    Cancelled(String),
}

#[zbus::interface(name = "org.freedesktop.PolicyKit1.Authority")]
impl FakePolkit {
    #[allow(clippy::type_complexity)]
    async fn check_authorization(
        &self,
        subject: (String, HashMap<String, OwnedValue>),
        action_id: String,
        _details: HashMap<String, String>,
        flags: u32,
        _cancellation_id: String,
    ) -> Result<(bool, bool, HashMap<String, String>), PolkitError> {
        let name: String = subject
            .1
            .get("name")
            .map(|v| String::try_from(v.clone()).unwrap())
            .unwrap_or_default();
        self.checks.lock().unwrap().push(Check {
            kind: subject.0,
            name,
            action: action_id,
            flags,
        });
        match *self.answer.lock().unwrap() {
            Answer::Allow => Ok((true, false, HashMap::new())),
            Answer::Deny => Ok((false, true, HashMap::new())),
            Answer::Cancel => Err(PolkitError::Cancelled("dismissed".into())),
        }
    }
}

struct Users;

impl Passwd for Users {
    fn uid_of(&self, name: &str) -> std::io::Result<Option<u32>> {
        Ok(match name {
            "alice" => Some(1000),
            "root" => Some(0),
            _ => None,
        })
    }
}

struct Setup {
    _bus: Bus,
    root: tempfile::TempDir,
    answer: Arc<Mutex<Answer>>,
    checks: Arc<Mutex<Vec<Check>>>,
    client: zbus::Connection,
    _helper: zbus::Connection,
    _polkit: zbus::Connection,
}

impl Setup {
    async fn new() -> Self {
        let bus = private_bus();
        let root = tempfile::tempdir().unwrap();
        let tokens = root
            .path()
            .join("usr/share/uq/themes/rendered/dracula-dark");
        fs::create_dir_all(&tokens).unwrap();
        fs::write(tokens.join("tokens.json"), "{}").unwrap();
        fs::create_dir_all(root.path().join("etc/uq")).unwrap();
        fs::write(
            root.path().join("etc/uq/greetd.toml"),
            "[default_session]\ncommand = \"/usr/lib/uq/uq-greeter-session\"\nuser = \"greeter\"\n",
        )
        .unwrap();

        let answer = Arc::new(Mutex::new(Answer::Allow));
        let checks = Arc::new(Mutex::new(Vec::new()));
        let polkit = connect(&bus).await;
        polkit
            .object_server()
            .at(
                "/org/freedesktop/PolicyKit1/Authority",
                FakePolkit {
                    answer: answer.clone(),
                    checks: checks.clone(),
                },
            )
            .await
            .unwrap();
        polkit
            .request_name("org.freedesktop.PolicyKit1")
            .await
            .unwrap();

        let helper_conn = connect(&bus).await;
        let helper = Arc::new(Helper::new(
            Config::with_root(root.path()),
            Arc::new(Polkit::new(helper_conn.clone())),
            Arc::new(Users),
        ));
        serve(&helper_conn, helper).await.unwrap();

        let client = connect(&bus).await;
        Self {
            _bus: bus,
            root,
            answer,
            checks,
            client,
            _helper: helper_conn,
            _polkit: polkit,
        }
    }

    async fn call<B>(&self, method: &str, body: &B) -> Result<(), zbus::Error>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
    {
        self.client
            .call_method(Some(NAME), PATH, Some(IFACE), method, body)
            .await
            .map(|_| ())
    }

    fn file(&self, rel: &str) -> std::path::PathBuf {
        self.root.path().join(rel)
    }
}

fn error_name(r: Result<(), zbus::Error>) -> String {
    match r {
        Err(zbus::Error::MethodError(name, _, _)) => name.to_string(),
        other => panic!("expected a method error, got {other:?}"),
    }
}

#[tokio::test]
async fn polkit_is_asked_about_the_callers_bus_name_with_interaction() {
    let s = Setup::new().await;
    s.call("SetLogindPowerActions", &("suspend", "lock", "poweroff"))
        .await
        .unwrap();
    let me = s.client.unique_name().unwrap().to_string();
    assert_eq!(
        *s.checks.lock().unwrap(),
        [Check {
            kind: "system-bus-name".into(),
            name: me,
            action: "org.uq.settings.power".into(),
            flags: 1,
        }]
    );
    let text = fs::read_to_string(s.file("etc/systemd/logind.conf.d/50-uq.conf")).unwrap();
    assert!(text.contains("HandleLidSwitchExternalPower=lock\n"));
}

#[tokio::test]
async fn denied_and_cancelled_are_not_authorized_and_write_nothing() {
    let s = Setup::new().await;
    for answer in [Answer::Deny, Answer::Cancel] {
        *s.answer.lock().unwrap() = answer;
        let r = s
            .call("SetLogindPowerActions", &("suspend", "lock", "poweroff"))
            .await;
        assert_eq!(error_name(r), "org.uq.SettingsHelper1.Error.NotAuthorized");
        let r = s.call("SetAutoLogin", &("alice",)).await;
        assert_eq!(error_name(r), "org.uq.SettingsHelper1.Error.NotAuthorized");
        let r = s
            .call("SetGreeterAppearance", &("dracula", "dark", false, false))
            .await;
        assert_eq!(error_name(r), "org.uq.SettingsHelper1.Error.NotAuthorized");
    }
    assert!(!s.file("etc/systemd").exists());
    assert!(!s.file("etc/uq/greeter.toml").exists());
    assert!(
        !fs::read_to_string(s.file("etc/uq/greetd.toml"))
            .unwrap()
            .contains("initial_session")
    );
    let actions: Vec<String> = s
        .checks
        .lock()
        .unwrap()
        .iter()
        .map(|c| c.action.clone())
        .collect();
    assert_eq!(
        actions,
        [
            "org.uq.settings.power",
            "org.uq.settings.autologin",
            "org.uq.settings.greeter"
        ]
        .repeat(2)
    );
}

#[tokio::test]
async fn polkit_missing_is_not_authorized() {
    let s = Setup::new().await;
    s._polkit
        .release_name("org.freedesktop.PolicyKit1")
        .await
        .unwrap();
    let r = s.call("ClearGreeterWallpaper", &()).await;
    assert_eq!(error_name(r), "org.uq.SettingsHelper1.Error.NotAuthorized");
}

#[tokio::test]
async fn bad_arguments_are_invalid_argument_and_never_reach_polkit() {
    let s = Setup::new().await;
    let r = s
        .call("SetLogindPowerActions", &("../../x", "lock", "poweroff"))
        .await;
    assert_eq!(
        error_name(r),
        "org.uq.SettingsHelper1.Error.InvalidArgument"
    );
    let r = s
        .call(
            "SetGreeterAppearance",
            &("../dracula", "dark", false, false),
        )
        .await;
    assert_eq!(
        error_name(r),
        "org.uq.SettingsHelper1.Error.InvalidArgument"
    );
    let r = s.call("SetAutoLogin", &("root",)).await;
    assert_eq!(
        error_name(r),
        "org.uq.SettingsHelper1.Error.InvalidArgument"
    );
    let r = s.call("SetGreeterMonitors", &("{\"outputs\":7}",)).await;
    assert_eq!(
        error_name(r),
        "org.uq.SettingsHelper1.Error.InvalidArgument"
    );
    // Wrong signature: rejected by the bus layer.
    let r = s.call("SetAutoLogin", &(5u32,)).await;
    assert!(r.is_err());
    assert!(s.checks.lock().unwrap().is_empty());
}

#[tokio::test]
async fn the_wallpaper_fd_is_passed_and_copied() {
    let s = Setup::new().await;
    let mut f = tempfile::tempfile().unwrap();
    f.write_all(b"\x89PNG not really").unwrap();
    let fd = zbus::zvariant::Fd::from(&f);
    s.call("SetGreeterWallpaper", &(fd,)).await.unwrap();
    assert_eq!(
        fs::read(s.file("var/lib/uq-greeter/wallpaper")).unwrap(),
        b"\x89PNG not really"
    );

    let dir = fs::File::open(s.root.path()).unwrap();
    let r = s
        .call("SetGreeterWallpaper", &(zbus::zvariant::Fd::from(&dir),))
        .await;
    assert_eq!(
        error_name(r),
        "org.uq.SettingsHelper1.Error.InvalidArgument"
    );

    s.call("ClearGreeterWallpaper", &()).await.unwrap();
    assert!(!s.file("var/lib/uq-greeter/wallpaper").exists());
}

#[tokio::test]
async fn auto_login_and_greeter_files_over_the_bus() {
    let s = Setup::new().await;
    s.call("SetAutoLogin", &("alice",)).await.unwrap();
    s.call("SetGreeterAppearance", &("dracula", "dark", true, false))
        .await
        .unwrap();
    s.call(
        "SetGreeterMonitors",
        &(r#"{"outputs":{"HDMI-A-1":{"mode":"preferred"}}}"#,),
    )
    .await
    .unwrap();
    let greetd = fs::read_to_string(s.file("etc/uq/greetd.toml")).unwrap();
    assert!(
        greetd.contains("[initial_session]\ncommand = \"/usr/bin/uq-session\"\nuser = \"alice\"\n")
    );
    assert!(
        fs::read_to_string(s.file("etc/uq/greeter.toml"))
            .unwrap()
            .contains("hide_users = true\n")
    );
    assert!(
        fs::read_to_string(s.file("etc/uq/greeter-monitors.toml"))
            .unwrap()
            .contains("[outputs.HDMI-A-1]\nmode = \"preferred\"\n")
    );
    s.call("SetAutoLogin", &("",)).await.unwrap();
    assert!(
        !fs::read_to_string(s.file("etc/uq/greetd.toml"))
            .unwrap()
            .contains("initial_session")
    );
}

#[tokio::test]
async fn introspection_lists_exactly_the_six_methods() {
    let s = Setup::new().await;
    let reply = s
        .client
        .call_method(
            Some(NAME),
            PATH,
            Some("org.freedesktop.DBus.Introspectable"),
            "Introspect",
            &(),
        )
        .await
        .unwrap();
    let xml: String = reply.body().deserialize().unwrap();
    let start = xml
        .find("<interface name=\"org.uq.SettingsHelper1\">")
        .unwrap();
    let end = start + xml[start..].find("</interface>").unwrap();
    let iface = &xml[start..end];
    let mut methods: Vec<&str> = iface
        .match_indices("<method name=\"")
        .map(|(i, m)| {
            let rest = &iface[i + m.len()..];
            &rest[..rest.find('"').unwrap()]
        })
        .collect();
    methods.sort();
    assert_eq!(
        methods,
        [
            "ClearGreeterWallpaper",
            "SetAutoLogin",
            "SetGreeterAppearance",
            "SetGreeterMonitors",
            "SetGreeterWallpaper",
            "SetLogindPowerActions"
        ]
    );
    assert!(!iface.contains("<property") && !iface.contains("<signal"));
}
