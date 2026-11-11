use super::*;
use std::fs;

const ROMFORGE: &str = r#"
[[window_rule]]
id = "romforge.emulators"
class = ["^(retroarch|duckstation-qt|PPSSPPSDL|dolphin-emu|pcsx2-qt|rpcs3|azahar)$"]
workspace = "name:game"
fullscreen = true
"#;

fn err(text: &str) -> String {
    parse_rules(text, "romforge").unwrap_err().to_string()
}

fn one(body: &str) -> String {
    format!("[[window_rule]]\nid = \"romforge.x\"\nclass = [\"^a$\"]\n{body}")
}

#[test]
fn spec_example_parses() {
    let rules = parse_rules(ROMFORGE, "romforge").unwrap();
    assert_eq!(rules.len(), 1);
    let r = &rules[0];
    assert_eq!(r.id, "romforge.emulators");
    assert_eq!(r.integration, "romforge");
    assert_eq!(r.workspace.as_deref(), Some("name:game"));
    assert_eq!((r.fullscreen, r.float), (Some(true), None));
    assert_eq!(r.name, None);
}

#[test]
fn every_key_parses() {
    let text = "uq_api = 1\n[[window_rule]]\nid = \"romforge.all\"\nname = \"All of it\"\n\
                class = [\"^a$\", \"^b$\"]\ntitle = \"^T$\"\nworkspace = 3\nfullscreen = false\n\
                float = true\n";
    let r = &parse_rules(text, "romforge").unwrap()[0];
    assert_eq!(r.name.as_deref(), Some("All of it"));
    assert_eq!(r.class, ["^a$", "^b$"]);
    assert_eq!(r.title.as_deref(), Some("^T$"));
    assert_eq!(r.workspace.as_deref(), Some("3"));
    assert_eq!((r.fullscreen, r.float), (Some(false), Some(true)));
}

#[test]
fn workspaces() {
    for (ws, want) in [
        ("\"name:game\"", "name:game"),
        ("\"name:My games_2-b\"", "name:My games_2-b"),
        ("\"7\"", "7"),
        ("1", "1"),
        ("100", "100"),
    ] {
        let r = parse_rules(&one(&format!("workspace = {ws}")), "romforge").unwrap();
        assert_eq!(r[0].workspace.as_deref(), Some(want), "{ws}");
    }
    for bad in [
        "\"game\"",
        "\"name:\"",
        "\"name:a/b\"",
        "\"name:x\\\"y\"",
        "\"name:a\\nb\"",
        "\"name: lead\"",
        &format!("\"name:{}\"", "a".repeat(33)),
        "\"special:magic\"",
        "\"e+1\"",
        "0",
        "101",
        "-1",
        "\"0\"",
        "1.5",
        "true",
    ] {
        let e = err(&one(&format!("workspace = {bad}")));
        assert!(e.contains("workspace"), "{bad}: {e}");
    }
}

#[test]
fn bad_regex_is_refused() {
    for bad in ["(", "[z-a]", "(?<=x)y", "\\1", ""] {
        let text = format!(
            "[[window_rule]]\nid = \"romforge.x\"\nclass = [{:?}]\nfullscreen = true\n",
            bad
        );
        let e = err(&text);
        assert!(
            e.contains("class[0]") && e.contains("pattern"),
            "{bad}: {e}"
        );
    }
    let e = err(&one("fullscreen = true\ntitle = \"[a\""));
    assert!(e.contains("title"), "{e}");
}

#[test]
fn unknown_keys_are_refused() {
    let e = err(&one("fullscreen = true\nexec = \"rm -rf\""));
    assert!(e.contains("exec"), "{e}");
    let e = err(&format!("lua = \"x\"\n{ROMFORGE}"));
    assert!(e.contains("lua"), "{e}");
    let e = err("[[layer_rule]]\nid = \"romforge.x\"\n");
    assert!(e.contains("layer_rule"), "{e}");
}

#[test]
fn id_must_carry_the_folder_name() {
    for bad in [
        "emobie.emulators",
        "emulators",
        "romforge.",
        "romforge.Big",
        "romforge.a.b",
        "romforge.a b",
        ".x",
    ] {
        let text = ROMFORGE.replace("romforge.emulators", bad);
        let e = err(&text);
        assert!(e.contains("romforge.<rule>"), "{bad}: {e}");
    }
    let twice = format!("{ROMFORGE}{ROMFORGE}");
    assert!(err(&twice).contains("twice"));
}

#[test]
fn limits_and_needs() {
    assert!(err("").contains("no [[window_rule]]"));
    let many: String = (0..17)
        .map(|i| {
            format!("[[window_rule]]\nid = \"romforge.r{i}\"\nclass = [\"a\"]\nfloat = true\n")
        })
        .collect();
    assert!(err(&many).contains("more than 16"));
    let nine = format!(
        "[[window_rule]]\nid = \"romforge.x\"\nclass = [{}]\nfloat = true\n",
        ["\"a\""; 9].join(", ")
    );
    assert!(err(&nine).contains("1 to 8"));
    let none = "[[window_rule]]\nid = \"romforge.x\"\nclass = []\nfloat = true\n";
    assert!(err(none).contains("1 to 8"));
    assert!(err(&one("")).contains("at least one"));
    let long = format!("a{}", "b".repeat(256));
    let e = err(&format!(
        "[[window_rule]]\nid = \"romforge.x\"\nclass = [\"{long}\"]\nfloat = true\n"
    ));
    assert!(e.contains("256"), "{e}");
    assert!(err(&one("float = true\nname = \"\"")).contains("name"));
    assert!(err(&one("float = true\nname = \"a\\u0007\"")).contains("name"));
    let e = err(&one("float = \"yes\""));
    assert!(e.contains("line 4") && e.contains("bool"), "{e}");
    assert!(err(&format!("uq_api = 2\n{ROMFORGE}")).contains("needs a newer uQ"));
    assert!(err(&format!("uq_api = \"1\"\n{ROMFORGE}")).contains("uq_api"));
}

#[test]
fn window_rules_one_per_pattern() {
    let mut r = parse_rules(ROMFORGE, "romforge").unwrap().remove(0);
    let w = r.window_rules();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].id, "app:romforge.emulators");
    assert!(w[0].enabled);
    assert_eq!(w[0].effects.workspace.as_deref(), Some("name:game"));
    assert_eq!(w[0].effects.fullscreen, Some(true));
    assert_eq!(w[0].effects.silent, None);
    r.class.push("^b$".into());
    r.title = Some("^t$".into());
    let ids: Vec<_> = r.window_rules().into_iter().map(|w| w.id).collect();
    assert_eq!(
        ids,
        ["app:romforge.emulators.1", "app:romforge.emulators.2"]
    );
    assert!(
        r.window_rules()
            .iter()
            .all(|w| w.matches.title.as_deref() == Some("^t$"))
    );
    let j = r.to_json();
    assert_eq!(j["name"], "romforge.emulators", "no name: the id");
    assert_eq!(j["integration"], "romforge");
    assert!(j["matches_text"].as_array().unwrap().len() >= 2, "{j}");
    assert!(!j["effects_text"].as_array().unwrap().is_empty(), "{j}");
}

fn put(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn scan_finds_files_and_reports_bad_ones() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("integrations");
    put(&root.join("romforge/rules.toml"), ROMFORGE);
    put(
        &root.join("emobie/rules.toml"),
        &ROMFORGE.replace("romforge.", "emobie."),
    );
    // The id names another folder.
    put(&root.join("thief/rules.toml"), ROMFORGE);
    put(&root.join("Bad_Name/rules.toml"), ROMFORGE);
    put(&root.join("hauler/README.md"), "no rules here");
    let files = scan_rules(&root).unwrap();
    let names: Vec<_> = files.iter().map(|f| f.integration.as_str()).collect();
    assert_eq!(names, ["Bad_Name", "emobie", "romforge", "thief"]);
    assert!(
        files[0]
            .rules
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("folder name")
    );
    assert!(files[3].rules.is_err());
    let ids: Vec<_> = installed(&root).into_iter().map(|r| r.id).collect();
    assert_eq!(ids, ["emobie.emulators", "romforge.emulators"]);
    assert!(scan_rules(&d.path().join("none")).unwrap().is_empty());
}

#[test]
fn scan_refuses_a_symlink_out_of_the_root() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("integrations");
    put(&d.path().join("elsewhere/rules.toml"), ROMFORGE);
    fs::create_dir_all(root.join("romforge")).unwrap();
    std::os::unix::fs::symlink(
        d.path().join("elsewhere/rules.toml"),
        root.join("romforge/rules.toml"),
    )
    .unwrap();
    let files = scan_rules(&root).unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].rules.is_err(), "{files:?}");
    assert!(installed(&root).is_empty());
}

#[test]
fn enabled_rules_keep_installed_order_and_skip_missing() {
    let installed = vec![
        parse_rules(&ROMFORGE.replace("romforge.", "emobie."), "emobie")
            .unwrap()
            .remove(0),
        parse_rules(ROMFORGE, "romforge").unwrap().remove(0),
    ];
    let on = IntegrationRules {
        enabled: vec!["romforge.emulators".into(), "gone.rule".into()],
    };
    let ids: Vec<_> = enabled_rules(&installed, &on)
        .into_iter()
        .map(|r| r.id)
        .collect();
    assert_eq!(ids, ["romforge.emulators"]);
}

#[test]
fn section_writes_are_strict() {
    let ok = IntegrationRules::from_section(&json!({"enabled": ["romforge.emulators"]})).unwrap();
    assert_eq!(ok.enabled, ["romforge.emulators"]);
    assert_eq!(ok.section(), json!({"enabled": ["romforge.emulators"]}));
    assert_eq!(
        IntegrationRules::from_section(&json!({})).unwrap(),
        IntegrationRules::default()
    );
    let many: Vec<String> = (0..65).map(|i| format!("a.r{i}")).collect();
    for bad in [
        json!([]),
        json!({"enabled": "romforge.emulators"}),
        json!({"enabled": ["nodot"]}),
        json!({"enabled": [1]}),
        json!({"enabled": ["a.b", "a.b"]}),
        json!({"enabled": ["a.b"], "more": 1}),
        json!({"enabled": many}),
    ] {
        assert!(IntegrationRules::from_section(&bad).is_err(), "{bad}");
    }
}

#[test]
fn loads_leniently_and_repairs() {
    let r: IntegrationRules =
        serde_json::from_value(json!({"enabled": ["a.b", 3, "bad", "a.b", "c.d"]})).unwrap();
    assert_eq!(r.clone().repaired().enabled, ["a.b", "c.d"]);
    let r: IntegrationRules = serde_json::from_value(json!({"enabled": 5})).unwrap();
    assert!(r.enabled.is_empty());
    let r: IntegrationRules = serde_json::from_value(json!("x")).unwrap();
    assert!(r.enabled.is_empty());
    let many = IntegrationRules {
        enabled: (0..70).map(|i| format!("a.r{i}")).collect(),
    };
    assert_eq!(many.repaired().enabled.len(), MAX_ENABLED);
}

#[test]
fn turned_on_and_summary() {
    let before = IntegrationRules {
        enabled: vec!["a.b".into()],
    };
    let after = IntegrationRules {
        enabled: vec!["a.b".into(), "c.d".into()],
    };
    assert_eq!(after.turned_on(&before), ["c.d"]);
    assert!(before.turned_on(&after).is_empty());
    assert_eq!(
        summary(&before.section(), &after.section()),
        "Rule from an app turned on: c.d"
    );
    assert_eq!(
        summary(&after.section(), &before.section()),
        "Rule from an app turned off: c.d"
    );
    assert_eq!(summary(&json!({}), &json!({})), "Rules from apps");
    let five = IntegrationRules {
        enabled: (0..5).map(|i| format!("a.r{i}")).collect(),
    };
    assert!(
        summary(&json!({}), &five.section()).ends_with(", and 2 more"),
        "{}",
        summary(&json!({}), &five.section())
    );
}
