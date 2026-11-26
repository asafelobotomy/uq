//! The input, rules, keybinds and startup sections over D-Bus.

use super::*;

#[tokio::test]
async fn input_section_round_trip_over_dbus() {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let generated = paths.generated_lua.clone();
    let comp = Arc::new(FakeCompositor::running());
    *comp.devices_json.lock().unwrap() =
        r#"{"mice":[{"name":"mx","defaultSpeed":0.25}],"keyboards":[{"name":"kb"}],"touch":[]}"#
            .into();
    let desktop = Desktop::new(paths, comp, Arc::new(FakeSystem::default())).unwrap();
    let _server = serve(
        zbus::connection::Builder::address(bus.address.as_str()),
        desktop,
    )
    .await
    .unwrap();
    let client = zbus::connection::Builder::address(bus.address.as_str())
        .build()
        .await
        .unwrap();
    let proxy = zbus::Proxy::new(&client, NAME, PATH, IFACE).await.unwrap();
    let mut changed = proxy.receive_signal("SectionChanged").await.unwrap();

    let () = proxy
        .call(
            "SetSection",
            &(
                "input",
                r#"{"keyboard":{"layouts":[{"layout":"us"},{"layout":"de"}]}}"#,
                "settings",
            ),
        )
        .await
        .unwrap();
    let got: String = proxy.call("GetSection", &("input",)).await.unwrap();
    let got: serde_json::Value = serde_json::from_str(&got).unwrap();
    assert_eq!(got["keyboard"]["layouts"][1]["layout"], "de");
    assert!(
        std::fs::read_to_string(&generated)
            .unwrap()
            .contains(r#"kb_layout = "us,de""#)
    );
    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), changed.next())
        .await
        .unwrap()
        .unwrap();
    let (domain, _): (String, String) = msg.body().deserialize().unwrap();
    assert_eq!(domain, "input");

    let devices: String = proxy.call("ListInputDevices", &()).await.unwrap();
    let devices: serde_json::Value = serde_json::from_str(&devices).unwrap();
    assert_eq!(devices["pointers"][0]["name"], "mx");
    assert_eq!(devices["keyboards"][0]["name"], "kb");

    let err = proxy
        .call::<_, _, ()>(
            "SetSection",
            &(
                "monitors",
                r#"{"outputs":{"DP-1":{"scale":1.5}}}"#,
                "settings",
            ),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("InvalidSetting"), "{err}");
}

#[tokio::test]
async fn rules_section_and_list_windows_over_dbus() {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let generated = paths.generated_lua.clone();
    let comp = Arc::new(FakeCompositor::running());
    *comp.clients_json.lock().unwrap() = r#"[{"address":"0x1","pid":7,"mapped":true,
        "class":"firefox","title":"Docs","initialClass":"firefox","initialTitle":"Docs",
        "workspace":{"id":2,"name":"2"},"floating":false,"xwayland":false,"pinned":false,
        "fullscreen":0,"tags":[]}]"#
        .into();
    let desktop = Desktop::new(paths, comp, Arc::new(FakeSystem::default())).unwrap();
    let _server = serve(
        zbus::connection::Builder::address(bus.address.as_str()),
        desktop,
    )
    .await
    .unwrap();
    let client = zbus::connection::Builder::address(bus.address.as_str())
        .build()
        .await
        .unwrap();
    let proxy = zbus::Proxy::new(&client, NAME, PATH, IFACE).await.unwrap();

    let rules = r#"{"window":[{"name":"Floaty","enabled":true,"match":{"class":"^pavucontrol$"},
        "effects":{"float":true}}],"layer":[],"workspace":[]}"#;
    let () = proxy
        .call("SetSection", &("rules", rules, "settings"))
        .await
        .unwrap();
    let got: String = proxy.call("GetSection", &("rules",)).await.unwrap();
    let got: serde_json::Value = serde_json::from_str(&got).unwrap();
    let id = got["window"][0]["id"].as_str().unwrap().to_string();
    assert!(
        std::fs::read_to_string(&generated)
            .unwrap()
            .contains(&format!("uq:{id}"))
    );
    let explained: String = proxy.call("Explain", &("rules",)).await.unwrap();
    let explained: serde_json::Value = serde_json::from_str(&explained).unwrap();
    assert_eq!(explained[format!("window.{id}")]["source"], "uq");

    let windows: String = proxy.call("ListWindows", &()).await.unwrap();
    let windows: serde_json::Value = serde_json::from_str(&windows).unwrap();
    assert_eq!(windows[0]["class"], "firefox");
    assert_eq!(windows[0]["workspace_id"], 2);
    assert!(windows[0].get("address").is_none() && windows[0].get("pid").is_none());
}

#[tokio::test]
async fn keybinds_section_and_list_binds_over_dbus() {
    let s = served().await;
    let kb = r#"{"overrides":[{"id":"uq.lookseedo","combo":"SUPER + K"}],"custom":[
        {"name":"Screenshot","combo":"SUPER + SHIFT + P",
         "action":{"kind":"command","command":"grim shot.png"}}]}"#;
    let () = s
        .proxy
        .call("SetSection", &("keybinds", kb, "settings"))
        .await
        .unwrap();
    let got: String = s.proxy.call("GetSection", &("keybinds",)).await.unwrap();
    let got: serde_json::Value = serde_json::from_str(&got).unwrap();
    let id = got["custom"][0]["id"].as_str().unwrap().to_string();
    assert!(
        std::fs::read_to_string(&s.paths.generated_lua)
            .unwrap()
            .contains(&format!("uq:{id}"))
    );
    // A conflict is InvalidSetting, naming both.
    let bad = r#"{"custom":[{"name":"Mine","combo":"SUPER + Return",
        "action":{"kind":"uq","name":"settings"}}]}"#;
    let err = s
        .proxy
        .call::<_, _, ()>("SetSection", &("keybinds", bad, "settings"))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("InvalidSetting"), "{err}");
    assert!(err.to_string().contains("Mine"), "{err}");
    let list: String = s.proxy.call("ListBinds", &()).await.unwrap();
    let list: serde_json::Value = serde_json::from_str(&list).unwrap();
    assert_eq!(list["binds"].as_array().unwrap().len(), 69);
    assert_eq!(list["binds"][5]["combo"], "SUPER + K");
    assert_eq!(list["limits"]["custom"], 100);
    let explained: String = s.proxy.call("Explain", &("keybinds",)).await.unwrap();
    let explained: serde_json::Value = serde_json::from_str(&explained).unwrap();
    assert_eq!(explained[format!("binds.{id}")]["source"], "uq");
}

#[tokio::test]
async fn startup_sections_over_dbus() {
    let s = served().await;
    let writes = [
        ("env", r#"{"vars":[{"name":"FOO","value":"bar"}]}"#),
        ("compat", r#"{"qt_wayland":true}"#),
        (
            "autostart",
            r#"{"files":[{"id":"nm-applet.desktop","enabled":false,"delay":null}],"added":[]}"#,
        ),
    ];
    for (domain, json) in writes {
        let () = s
            .proxy
            .call("SetSection", &(domain, json, "settings"))
            .await
            .unwrap();
        let got: String = s.proxy.call("GetSection", &(domain,)).await.unwrap();
        let got: serde_json::Value = serde_json::from_str(&got).unwrap();
        let sent: serde_json::Value = serde_json::from_str(json).unwrap();
        for (k, v) in sent.as_object().unwrap() {
            assert_eq!(&got[k], v, "{domain}.{k}");
        }
    }
    let g = std::fs::read_to_string(&s.paths.generated_lua).unwrap();
    assert!(g.contains(r#"hl.env("FOO", "bar", true)"#), "{g}");
    assert!(
        g.contains(r#"hl.env("QT_QPA_PLATFORM", "wayland;xcb", true)"#),
        "{g}"
    );
    let env_d = s.paths.startup.config_home.join("environment.d/60-uq.conf");
    let env_d = std::fs::read_to_string(env_d).unwrap();
    assert!(env_d.contains("FOO='bar'"), "{env_d}");

    let explain = |domain: &'static str| {
        let proxy = s.proxy.clone();
        async move {
            let e: String = proxy.call("Explain", &(domain,)).await.unwrap();
            serde_json::from_str::<serde_json::Value>(&e).unwrap()
        }
    };
    assert_eq!(explain("env").await["vars.FOO"]["source"], "uq");
    let compat = explain("compat").await;
    assert_eq!(compat["qt_wayland"]["source"], "uq");
    assert_eq!(compat["sharp_x11"]["source"], "default");
    assert_eq!(
        explain("autostart").await["nm-applet.desktop"]["source"],
        "uq"
    );

    let err = s
        .proxy
        .call::<_, _, ()>(
            "SetSection",
            &(
                "compat",
                r#"{"apps":[{"id":"code.desktop","platform":"x11","elsewhere":true}]}"#,
                "settings",
            ),
        )
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidSetting");
    assert_eq!(error_message(&err), "uQ can't find this app's file.");
}
