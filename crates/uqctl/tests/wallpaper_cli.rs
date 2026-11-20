//! `uqctl wallpaper` end to end: the real binary against a real uqd on a private bus, with the
//! home under a temporary directory. Never touches the user's session.

mod common;

use common::{connect, private_bus, serve_uqd};
use uq_core::client::DesktopProxy;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_shows_in_the_section_and_undoes() {
    let bus = private_bus();
    let root = tempfile::tempdir().unwrap();
    let _uqd = serve_uqd(&bus, root.path()).await;
    let pic = root.path().join("sea shore.png");
    std::fs::write(&pic, b"not really a png").unwrap();
    let uqctl = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_uqctl"))
            .args(args)
            .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
            .env_remove("WAYLAND_DISPLAY")
            .output()
            .unwrap()
    };
    let err = |o: &std::process::Output| String::from_utf8_lossy(&o.stderr).into_owned();

    let o = uqctl(&[
        "wallpaper",
        "set",
        "--fit",
        "fit",
        "--",
        pic.to_str().unwrap(),
    ]);
    assert!(o.status.success(), "{}", err(&o));
    let proxy = DesktopProxy::new(&connect(&bus).await).await.unwrap();
    let section: serde_json::Value =
        serde_json::from_str(&proxy.get_section("wallpaper").await.unwrap()).unwrap();
    let all = &section["outputs"]["*"];
    assert_eq!(all["image"], pic.canonicalize().unwrap().to_str().unwrap());
    assert_eq!(all["fit"], "fit");
    assert!(all.get("folder").is_none());

    let o = uqctl(&["wallpaper", "get"]);
    assert!(o.status.success(), "{}", err(&o));
    assert!(String::from_utf8_lossy(&o.stdout).contains("sea shore.png\tfit"));

    let o = uqctl(&["wallpaper", "set", "--", "/no/such/picture.png"]);
    assert!(!o.status.success());
    assert!(
        err(&o).contains("Can't open /no/such/picture.png"),
        "{}",
        err(&o)
    );

    let o = uqctl(&["undo"]);
    assert!(o.status.success(), "{}", err(&o));
    let section: serde_json::Value =
        serde_json::from_str(&proxy.get_section("wallpaper").await.unwrap()).unwrap();
    assert!(section["outputs"]["*"].get("image").is_none(), "{section}");
}
