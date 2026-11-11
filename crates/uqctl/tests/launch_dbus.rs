//! End-to-end launches: the real `uqctl` binary (as `bridge` and as `launch`) against a private
//! bus, a fake Hyprland socket and a temporary data dir. Never touches the user's session.

mod common;

use common::{Bridge, Bus, FakeHypr, connect, private_bus, serve_uqd};
use serde_json::json;
use std::ffi::{OsStr, OsString};
use std::time::{Duration, Instant};
use uq_core::client::DesktopProxy;

const CODE: &str = "[Desktop Entry]\nExec=code %F\nName=Code\n";
const FIX: &str = "GDK_BACKEND=wayland,x11";

/// A data home (empty) and one data dir holding `code.desktop`.
fn data_dirs() -> (tempfile::TempDir, Vec<(&'static str, OsString)>) {
    let t = tempfile::tempdir().unwrap();
    let apps = t.path().join("sys/applications");
    std::fs::create_dir_all(&apps).unwrap();
    std::fs::create_dir_all(t.path().join("home/applications")).unwrap();
    std::fs::write(apps.join("code.desktop"), CODE).unwrap();
    let env = vec![
        ("XDG_DATA_HOME", t.path().join("home").into()),
        ("XDG_DATA_DIRS", t.path().join("sys").into()),
    ];
    (t, env)
}

fn refs<'a>(env: &'a [(&'static str, OsString)]) -> Vec<(&'static str, &'a OsStr)> {
    env.iter().map(|(k, v)| (*k, v.as_os_str())).collect()
}

/// Start a bridge that sees the fake Hyprland and the temporary data dirs.
async fn bridge(bus: &Bus, hypr: &FakeHypr, data: &[(&'static str, OsString)]) -> Bridge {
    let mut env = hypr.env();
    env.extend(data.iter().cloned());
    let mut b = Bridge::start_with_env(bus, &refs(&env));
    b.event("Hello").await;
    b
}

async fn set_fix(bus: &Bus) {
    let conn = connect(bus).await;
    let proxy = DesktopProxy::new(&conn).await.unwrap();
    let compat = json!({"apps": [{"id": "code.desktop", "platform": "wayland", "args": ["--x"]}]});
    proxy
        .set_section("compat", &compat.to_string(), "settings")
        .await
        .unwrap();
}

#[tokio::test]
async fn launch_fetches_compat_fix() {
    let bus = private_bus();
    let state = tempfile::tempdir().unwrap();
    let _uqd = serve_uqd(&bus, state.path()).await;
    set_fix(&bus).await;
    let mut hypr = FakeHypr::start();
    let (_data, data) = data_dirs();
    let mut b = bridge(&bus, &hypr, &data).await;

    let r = b.call(1, "Launch", json!([{"desktop_id": "code"}])).await;
    assert_eq!(r["ok"], serde_json::Value::Null, "{r}");
    let sent = hypr.next(Duration::from_secs(5)).await.expect("a dispatch");
    assert!(sent.contains(&format!("'env' '--' '{FIX}'")), "{sent}");
    assert!(sent.contains("'code' '--x'"), "{sent}");
}

#[tokio::test]
async fn launch_with_uqd_down_still_launches() {
    let bus = private_bus();
    let mut hypr = FakeHypr::start();
    let (_data, data) = data_dirs();
    let mut b = bridge(&bus, &hypr, &data).await;

    let r = b.call(1, "Launch", json!([{"desktop_id": "code"}])).await;
    assert_eq!(r["ok"], serde_json::Value::Null, "{r}");
    let sent = hypr.next(Duration::from_secs(5)).await.expect("a dispatch");
    assert!(sent.ends_with("-- 'code'\"))"), "{sent}");
}

/// A uqd that takes 2 s to answer `GetSection`.
struct SlowUqd;

#[zbus::interface(name = "org.uq.Desktop1")]
impl SlowUqd {
    async fn get_section(&self, _domain: &str) -> String {
        tokio::time::sleep(Duration::from_secs(2)).await;
        json!({"apps": [{"id": "code.desktop", "platform": "wayland"}]}).to_string()
    }

    async fn record_launch(&self, _id: &str) {}
}

#[tokio::test]
async fn launch_with_slow_uqd_times_out_and_launches() {
    let bus = private_bus();
    let _slow = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .name("org.uq.Desktop1")
        .unwrap()
        .serve_at("/org/uq/Desktop1", SlowUqd)
        .unwrap()
        .build()
        .await
        .unwrap();
    let mut hypr = FakeHypr::start();
    let (_data, data) = data_dirs();
    let mut b = bridge(&bus, &hypr, &data).await;

    let t = Instant::now();
    b.send(1, "Launch", json!([{"desktop_id": "code"}])).await;
    let sent = hypr.next(Duration::from_secs(5)).await.expect("a dispatch");
    let took = t.elapsed();
    // It asked uqd, gave up at 500 ms and launched without the fix.
    assert!(
        took >= Duration::from_millis(450),
        "dispatched after {took:?}"
    );
    assert!(
        took < Duration::from_millis(1200),
        "dispatched after {took:?}"
    );
    assert!(!sent.contains(FIX), "{sent}");
    assert_eq!(b.reply(1).await["ok"], serde_json::Value::Null);
}

fn uqctl(bus: &Bus, env: &[(&'static str, OsString)], args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_uqctl"))
        .args(args)
        .envs(env.iter().map(|(k, v)| (k, v)))
        .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap()
}

#[tokio::test]
async fn uqctl_launch_unknown_id_fails_plainly() {
    let bus = private_bus();
    let (_data, mut env) = data_dirs();
    let none = tempfile::tempdir().unwrap();
    env.push(("XDG_RUNTIME_DIR", none.path().into()));
    env.push(("HYPRLAND_INSTANCE_SIGNATURE", "SIG".into()));
    let out = uqctl(&bus, &env, &["launch", "nope"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(err, "uqctl: no app called nope\n");
    assert!(out.stdout.is_empty());

    // A known app with Hyprland not running fails plainly too.
    let out = uqctl(&bus, &env, &["launch", "code"]);
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "uqctl: Hyprland is not running\n"
    );
}

#[tokio::test]
async fn uqctl_launch_applies_fix_and_records() {
    let bus = private_bus();
    let state = tempfile::tempdir().unwrap();
    let _uqd = serve_uqd(&bus, state.path()).await;
    set_fix(&bus).await;
    let mut hypr = FakeHypr::start();
    let (_data, mut env) = data_dirs();
    env.extend(hypr.env());
    let run = {
        let (bus_addr, env) = (bus.address.clone(), env.clone());
        tokio::task::spawn_blocking(move || {
            std::process::Command::new(env!("CARGO_BIN_EXE_uqctl"))
                .args(["launch", "code"])
                .envs(env.iter().map(|(k, v)| (k, v)))
                .env("DBUS_SESSION_BUS_ADDRESS", bus_addr)
                .output()
                .unwrap()
        })
    };
    let sent = hypr
        .next(Duration::from_secs(10))
        .await
        .expect("a dispatch");
    let out = run.await.unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(sent.contains(&format!("'env' '--' '{FIX}'")), "{sent}");
    assert!(sent.contains("--unit=app-uq-code-"), "{sent}");
    let conn = connect(&bus).await;
    let counts = DesktopProxy::new(&conn)
        .await
        .unwrap()
        .get_launch_counts()
        .await
        .unwrap();
    assert_eq!(counts.get("code.desktop"), Some(&1), "{counts:?}");
}
