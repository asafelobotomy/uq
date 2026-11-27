//! Validating the startup trees.

use super::*;

#[test]
fn env_tree_validation() {
    let p = Profile::parse(&with("[env]\nFOO = \"bar\"\nOLD = false\n")).unwrap();
    assert_eq!(p.values["env"], json!({"FOO": "bar", "OLD": false}));
    assert_eq!(Profile::parse(&p.to_toml()), Ok(p));
    let long = format!("[env]\nA = \"{}\"\n", "x".repeat(4097));
    for (body, key) in [
        ("[env]\nfoo = \"x\"\n", "env.foo"),
        ("[env]\nXDG_SESSION_TYPE = \"x\"\n", "env.XDG_SESSION_TYPE"),
        ("[env]\nXDG_CONFIG_HOME = \"x\"\n", "env.XDG_CONFIG_HOME"),
        ("[env]\nUQ_X = false\n", "env.UQ_X"),
        ("[env]\nA = \"$HOME\"\n", "env.A"),
        ("[env]\nA = \"a\\u0001b\"\n", "env.A"),
        (long.as_str(), "env.A"),
        ("[env]\nA = true\n", "env.A"),
        ("[env]\nA = 3\n", "env.A"),
    ] {
        assert_eq!(refused(body).0, key, "{body:?}");
    }
    assert!(
        refused("[env]\nA = true\n")
            .1
            .starts_with("Environment variable A: ")
    );
    assert!(Profile::parse(&with("[env]\nLD_PRELOAD = \"x\"\n")).is_ok());
}

#[test]
fn compat_tree_validation() {
    let ok = "[compat]\nsharp_x11 = true\nqt_wayland = false\n\
              [compat.apps.\"code.desktop\"]\nplatform = \"wayland\"\nscale = 1.5\n";
    let p = Profile::parse(&with(ok)).unwrap();
    assert_eq!(Profile::parse(&p.to_toml()), Ok(p));
    let app = |field: &str| format!("[compat.apps.\"code.desktop\"]\n{field}\n");
    for (body, key) in [
        (app("scale = 1.1"), "compat.apps.code.desktop.scale"),
        (
            app("platform = \"mir\""),
            "compat.apps.code.desktop.platform",
        ),
        (app("env = []"), "compat.apps.code.desktop.env"),
        (app("args = [\"--x\"]"), "compat.apps.code.desktop.args"),
        (
            app("elsewhere = true"),
            "compat.apps.code.desktop.elsewhere",
        ),
        (
            "[compat.apps.\"../x\"]\nplatform = \"x11\"\n".to_string(),
            "compat.apps.../x",
        ),
        ("[compat]\nblur = true\n".to_string(), "compat.blur"),
    ] {
        let (k, text) = refused(&body);
        assert_eq!(k, key, "{body:?}");
        assert!(!text.contains(".desktop"), "{text}");
    }
    let (_, text) = refused(&app("args = [\"--x\"]"));
    assert!(
        text.contains("a profile can only set an app's Run on and Size"),
        "{text}"
    );
}

#[test]
fn autostart_tree_validation() {
    for ok in [
        "[autostart.\"nm-applet.desktop\"]\nenabled = false\n",
        "[autostart.\"a-3f9c\"]\ndelay = 10\n",
    ] {
        let p = Profile::parse(&with(ok)).unwrap();
        assert_eq!(Profile::parse(&p.to_toml()), Ok(p));
    }
    let e = |field: &str| format!("[autostart.\"a-3f9c\"]\n{field}\n");
    for (body, key) in [
        (e("argv = [\"x\"]"), "autostart.a-3f9c.argv"),
        (e("app = \"x.desktop\""), "autostart.a-3f9c.app"),
        (e("name = \"x\""), "autostart.a-3f9c.name"),
        (e("delay = 601"), "autostart.a-3f9c.delay"),
        (e("enabled = \"no\""), "autostart.a-3f9c.enabled"),
        (
            "[autostart.\"../x\"]\nenabled = true\n".to_string(),
            "autostart.../x",
        ),
        (
            "[autostart.\"a-ZZ\"]\nenabled = true\n".to_string(),
            "autostart.a-ZZ",
        ),
    ] {
        assert_eq!(refused(&body).0, key, "{body:?}");
    }
    let (_, text) = refused(&e("argv = [\"x\"]"));
    assert!(
        text.contains(
            "a profile can only turn a startup app on or off, set its delay, or start it hidden"
        ),
        "{text}"
    );
}
