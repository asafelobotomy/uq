use super::*;
use serde_json::json;

const GAMING: &str = r##"format = 1
id = "gaming"
name = "Gaming"
icon = "gamepad"

[look]
gaps_in = 0
gaps_out = 0
blur = false

[shell.islands]
time = false

[appearance]
variant = "dark"

[power]
profile = "performance"
"##;

fn err(text: &str) -> ProfileError {
    Profile::parse(text).unwrap_err()
}

fn with(body: &str) -> String {
    format!("format = 1\nid = \"x\"\nname = \"X\"\n{body}")
}

#[test]
fn parse_spec_example() {
    let p = Profile::parse(GAMING).unwrap();
    assert_eq!(p.id, "gaming");
    assert_eq!(p.name, "Gaming");
    assert_eq!(p.icon.as_deref(), Some("gamepad"));
    assert_eq!(p.keys().len(), 6);
    assert_eq!(p.power(), Some("performance"));
    assert_eq!(p.values["look"]["gaps_in"], json!(0));
}

#[test]
fn to_toml_round_trips() {
    let mut p = Profile::parse(GAMING).unwrap();
    p.with_key(Key {
        domain: "wallpaper".into(),
        path: ["outputs", "HDMI-A.1", "image"].map(String::from).to_vec(),
        value: json!("/a.png"),
    });
    p.with_key(Key {
        domain: "look".into(),
        path: vec!["animation_speed".into()],
        value: json!(2.0),
    });
    let text = p.to_toml();
    assert!(text.starts_with(HEADER), "{text}");
    assert!(text.contains("format = 1"), "{text}");
    assert!(text.contains("HDMI-A.1"), "{text}");
    assert_eq!(Profile::parse(&text), Ok(p));
}

#[test]
fn refuses_missing_or_newer_format() {
    assert_eq!(
        err("id = \"x\"\nname = \"X\"\n[look]\ngaps_in = 1\n"),
        ProfileError::NotAProfile
    );
    assert_eq!(
        err("format = 2\nid = \"x\"\nname = \"X\"\n[look]\ngaps_in = 1\n"),
        ProfileError::NewerFormat(2)
    );
    assert_eq!(err("this is [not toml"), ProfileError::NotAProfile);
}

#[test]
fn refuses_unknown_domain() {
    for d in ["monitors", "input", "settings"] {
        let e = err(&with(&format!("[look]\ngaps_in = 1\n[{d}]\na = 1\n")));
        assert_eq!(e, ProfileError::UnknownDomain(d.into()), "{d}");
    }
}

#[test]
fn keybinds_domain_holds_shortcut_switches() {
    let p = Profile::parse(&with("[keybinds]\n\"uq.lookseedo\" = false\n")).unwrap();
    assert_eq!(p.values["keybinds"], json!({"uq.lookseedo": false}));
    // The "newer version" refusal stays for a future domain; no domain uses it now.
    assert!(
        ProfileError::NeedsNewer("keybinds".into())
            .to_string()
            .contains("keyboard shortcuts")
    );
}

#[test]
fn refuses_invalid_values() {
    let e = err(&with("[look]\ngaps_in = 999\n"));
    assert!(
        matches!(&e, ProfileError::Invalid { key, .. } if key == "look.gaps_in"),
        "{e:?}"
    );
    assert!(e.to_string().starts_with("Gaps between windows: "), "{e}");
    assert!(matches!(
        err(&with("[look]\nbogus = 1\n")),
        ProfileError::Invalid { .. }
    ));
    let e = err(&with("[power]\nprofile = \"turbo\"\n"));
    assert!(
        matches!(&e, ProfileError::Invalid { key, .. } if key == "power.profile"),
        "{e:?}"
    );
    let e = err(&with("[appearance]\nvariant = \"dim\"\n"));
    assert!(
        matches!(&e, ProfileError::Invalid { key, .. } if key == "appearance.variant"),
        "{e:?}"
    );
    assert!(matches!(
        err(&with("[power]\nprofile = \"balanced\"\nother = 1\n")),
        ProfileError::Invalid { .. }
    ));
}

#[test]
fn refuses_hostile_files() {
    let big = "# padding padding padding\n".repeat(65 * 1024 / 25 + 10);
    assert!(big.len() > 65_536);
    assert_eq!(err(&big), ProfileError::TooBig);
    assert_eq!(
        err("format = 1\nid = \"../x\"\nname = \"X\"\n[look]\ngaps_in = 1\n"),
        ProfileError::BadId
    );
    let long = "n".repeat(101);
    for name in [long.as_str(), "bell\u{7}", ""] {
        let t = format!(
            "format = 1\nid = \"x\"\nname = {}\n[look]\ngaps_in = 1\n",
            toml::Value::String(name.into())
        );
        assert_eq!(err(&t), ProfileError::BadName, "{name:?}");
    }
    assert_eq!(
        err(&with("icon = \"a/b\"\n[look]\ngaps_in = 1\n")),
        ProfileError::BadIcon
    );
    let deep_header = format!("[{}]\na = 1\n", vec!["a"; 5000].join("."));
    assert!(Profile::parse(&with(&deep_header)).is_err());
    let inline = format!("{}1{}", "{a=".repeat(5000), "}".repeat(5000));
    assert!(Profile::parse(&with(&format!("[look]\ngaps_in = {inline}\n"))).is_err());
    let arrays = format!("{}{}", "[".repeat(5000), "]".repeat(5000));
    assert!(Profile::parse(&with(&format!("[look]\ngaps_in = {arrays}\n"))).is_err());
}

#[test]
fn errors_never_repeat_control_characters_from_the_file() {
    for body in [
        "[\"\\u001b[31mx\"]\na = 1\n",
        "[look]\n\"\\u001b]0;pwn\\u0007\" = 1\n",
        "[look]\ngaps_in = \"\\u001b[2J\"\n",
        "[shell.\"\\u009b6n\"]\na = 1\n",
    ] {
        let text = err(&with(body)).to_string();
        assert!(
            !text.chars().any(char::is_control),
            "{body:?} gave {text:?}"
        );
        assert!(text.len() > 10, "{text:?}");
    }
}

#[test]
fn too_many_keys_is_refused() {
    let mut p = Profile::parse(&with("[look]\ngaps_in = 1\n")).unwrap();
    p.values.insert(
        "shell".into(),
        serde_json::from_str(&format!(
            "{{{}}}",
            (0..501)
                .map(|i| format!("\"k{i}\":1"))
                .collect::<Vec<_>>()
                .join(",")
        ))
        .unwrap(),
    );
    assert_eq!(p.validate(), Err(ProfileError::TooManyKeys));
}

#[test]
fn profile_holds_startup_domains() {
    let body = "[env]\nFOO = \"bar\"\nOLD = false\n[compat]\nsharp_x11 = true\n\
                [compat.apps.\"code.desktop\"]\nplatform = \"x11\"\nscale = 1.5\n\
                [autostart.\"nm-applet.desktop\"]\nenabled = false\n\
                [autostart.\"a-3f9c\"]\ndelay = 10\n";
    let p = Profile::parse(&with(body)).unwrap();
    assert_eq!(p.keys().len(), 7);
    assert_eq!(Profile::parse(&p.to_toml()), Ok(p.clone()));
    // 501 keys across the three is too many, as anywhere.
    let mut big = p;
    let vars: Map<String, Value> = (0..300).map(|i| (format!("V{i}"), json!("x"))).collect();
    big.values.insert("env".into(), Value::Object(vars));
    let ids: Map<String, Value> = (0..197)
        .map(|i| (format!("e{i}.desktop"), json!({"enabled": true})))
        .collect();
    big.values.insert("autostart".into(), Value::Object(ids));
    assert_eq!(big.keys().len(), 300 + 3 + 197);
    assert_eq!(big.validate(), Ok(()));
    let mut over = big.clone();
    over.with_key(Key {
        domain: "autostart".into(),
        path: vec!["e0.desktop".into(), "delay".into()],
        value: json!(1),
    });
    assert_eq!(over.validate(), Err(ProfileError::TooManyKeys));
}

#[test]
fn empty_profile_is_refused() {
    assert_eq!(err(&with("")), ProfileError::Empty);
    assert_eq!(err(&with("[look]\n")), ProfileError::Empty);
}

#[test]
fn write_leaves_sets_and_removes() {
    let mut st = State::default();
    write_leaves(
        &mut st,
        "appearance",
        &[
            (vec!["theme".into()], Some(json!("my-theme"))),
            (vec!["variant".into()], Some(json!("light"))),
            (vec!["font_size".into()], Some(json!(12))),
        ],
    )
    .unwrap();
    assert_eq!(st.theme, "my-theme");
    assert_eq!(st.variant, Variant::Light);
    assert_eq!(
        read_leaf(&st, "appearance", &["font_size".into()]),
        Some(json!(12))
    );
    assert_eq!(
        read_leaf(&st, "appearance", &["theme".into()]),
        Some(json!("my-theme"))
    );
    write_leaves(&mut st, "appearance", &[(vec!["font_size".into()], None)]).unwrap();
    assert_eq!(read_leaf(&st, "appearance", &["font_size".into()]), None);
    let before = st.clone();
    assert!(
        write_leaves(
            &mut st,
            "appearance",
            &[(vec!["theme".into()], Some(json!("../x")))]
        )
        .is_err()
    );
    assert!(
        write_leaves(
            &mut st,
            "look",
            &[(vec!["gaps_in".into()], Some(json!(999)))]
        )
        .is_err()
    );
    assert_eq!(st, before);
}

#[test]
fn id_for_name_slugs_and_dedups() {
    assert_eq!(id_for_name("Gaming", &[]), "gaming");
    assert_eq!(id_for_name("Gaming", &["gaming".into()]), "gaming-2");
    assert_eq!(
        id_for_name("Gaming", &["gaming".into(), "gaming-2".into()]),
        "gaming-3"
    );
    assert_eq!(id_for_name("!!!", &[]), "profile");
    assert_eq!(id_for_name("  Work & Play!  ", &[]), "work-play");
    let long = id_for_name(&"a".repeat(80), &[]);
    assert_eq!(long.len(), 32);
    assert!(valid_id(&id_for_name(&"a".repeat(80), &[long])));
    assert!(!valid_id("../x") && !valid_id("") && !valid_id("A") && !valid_id("-a"));
}

#[test]
fn key_label_names_every_kind() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(key_label("appearance", &s(&["theme"])), "Theme");
    assert_eq!(key_label("appearance", &s(&["variant"])), "Light or dark");
    assert_eq!(key_label("power", &s(&["profile"])), "Power mode");
    assert_eq!(key_label("look", &s(&["gaps_in"])), "Gaps between windows");
    assert_eq!(
        key_label("shell", &s(&["islands", "time"])),
        "Bar island: Clock"
    );
    assert_eq!(power_label("power-saver"), "Power saver");
    assert_eq!(power_label("balanced"), "Balanced");
    assert_eq!(power_label("performance"), "Performance");
    assert_eq!(value_text("look", &s(&["blur"]), &json!(true)), "On");
    assert_eq!(value_text("look", &s(&["blur"]), &json!(false)), "Off");
    assert_eq!(value_text("look", &s(&["gaps_in"]), &json!(5)), "5");
    assert_eq!(
        value_text("shell", &s(&["providers"]), &json!(["a", "b"])),
        "2 items"
    );
    assert_eq!(
        value_text("power", &s(&["profile"]), &json!("balanced")),
        "Balanced"
    );
    assert_eq!(
        value_text("appearance", &s(&["ui_font"]), &json!("Inter")),
        "Inter"
    );
    assert_eq!(
        power_notice("it isn't available"),
        "Power mode wasn't changed: it isn't available."
    );
}

#[test]
fn leaves_descends_objects_only() {
    let v = json!({"a": {"b": 1, "c": []}, "d": {}, "e": [1, 2]});
    let got = leaves(&v);
    assert_eq!(
        got,
        vec![
            (vec!["a".to_string(), "b".into()], json!(1)),
            (vec!["a".to_string(), "c".into()], json!([])),
            (vec!["e".to_string()], json!([1, 2])),
        ]
    );
}

#[test]
fn without_key_prunes_empty_domains() {
    let mut p = Profile::parse(GAMING).unwrap();
    let path = vec!["profile".to_string()];
    assert!(p.without_key("power", &path));
    assert!(!p.values.contains_key("power"));
    assert!(!p.without_key("power", &path));
    assert_eq!(p.power(), None);
}

#[test]
fn read_file_refuses_big_and_binary_files() {
    let dir = tempfile::tempdir().unwrap();
    let big = dir.path().join("big.toml");
    std::fs::write(&big, vec![b'#'; 1 << 20]).unwrap();
    assert_eq!(read_file(&big), Err(ProfileError::TooBig));
    let bin = dir.path().join("bin.toml");
    std::fs::write(&bin, [0xff, 0xfe, 0x00, 0x80]).unwrap();
    assert_eq!(read_file(&bin), Err(ProfileError::NotAProfile));
    let ok = dir.path().join("ok.toml");
    std::fs::write(&ok, GAMING).unwrap();
    assert_eq!(read_file(&ok).unwrap(), GAMING);
    assert!(matches!(
        read_file(&dir.path().join("none.toml")),
        Err(ProfileError::Unreadable(_))
    ));
}
