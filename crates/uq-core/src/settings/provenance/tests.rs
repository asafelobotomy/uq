use super::*;
use serde_json::json;

#[test]
fn finds_dotted_path() {
    let text = "-- mine\nlocal x = 1\nhl.config({ general.gaps_in = 2 })\n";
    assert_eq!(find_in_user_lua(text, "general:gaps_in"), Some(3));
    let text = "x = decoration.blur.enabled\n";
    assert_eq!(find_in_user_lua(text, "decoration:blur:enabled"), Some(1));
    assert_eq!(
        find_in_user_lua("general.gaps_inner = 1", "general:gaps_in"),
        None
    );
}

#[test]
fn finds_rule_name_in_user_lua() {
    let text = "-- hl.window_rule({ name = \"uq:r-7f2c\" })\nx = 1 -- uq:r-7f2c\"\n\
        hl.window_rule({ name = \"uq:r-7f2c0\", enabled = false })\n\
        hl.window_rule({ name = \"uq:r-7f2c\", enabled = false })\n";
    assert_eq!(find_rule_in_user_lua(text, "r-7f2c"), Some(4));
    assert_eq!(find_rule_in_user_lua("n = 'uq:r-7f2c'", "r-7f2c"), Some(1));
    assert_eq!(
        find_rule_in_user_lua("n = [[uq:r-7f2c]]", "r-7f2c"),
        Some(1)
    );
    assert_eq!(find_rule_in_user_lua("n = \"uq:r-7f2c0\"", "r-7f2c"), None);
    assert_eq!(find_rule_in_user_lua("n = \"uq:r-7f2c", "r-7f2c"), None);
    assert_eq!(find_rule_in_user_lua("-- \"uq:r-7f2c\"", "r-7f2c"), None);
}

#[test]
fn finds_workspace_rule_line() {
    let text = "-- hl.workspace_rule({ workspace = \"3\" })\nx = \"3\"\n\
        hl.workspace_rule({ workspace = \"3\", persistent = false })\n\
        hl.workspace_rule({ workspace = 'name:web' })\n";
    assert_eq!(find_workspace_rule_in_user_lua(text, "3"), Some(3));
    assert_eq!(find_workspace_rule_in_user_lua(text, "name:web"), Some(4));
    assert_eq!(find_workspace_rule_in_user_lua(text, "4"), None);
}

#[test]
fn finds_colon_path() {
    let text = "\n\nhl.keyword(\"general:gaps_out\", 9)\n";
    assert_eq!(find_in_user_lua(text, "general:gaps_out"), Some(3));
    assert_eq!(find_in_user_lua(text, "general:gaps_in"), None);
}

#[test]
fn finds_leaf_inside_section_table() {
    let text = "hl.config({\n    general = {\n        gaps_in = 2,\n    },\n})\n";
    assert_eq!(find_in_user_lua(text, "general:gaps_in"), Some(3));
    let text = "hl.config({\n  decoration = {\n    rounding = 4,\n    blur = { size = 3, enabled = false },\n  },\n})\n";
    assert_eq!(find_in_user_lua(text, "decoration:blur:enabled"), Some(4));
    assert_eq!(find_in_user_lua(text, "decoration:rounding"), Some(3));
    let one = "hl.config({ general = { layout = \"master\" } })";
    assert_eq!(find_in_user_lua(one, "general:layout"), Some(1));
}

#[test]
fn ignores_leaf_in_other_table() {
    let text = "hl.config({\n  decoration = {\n    gaps_in = 2,\n  },\n  general = { border_size = 1 },\n  gaps_in = 3,\n})\n";
    assert_eq!(find_in_user_lua(text, "general:gaps_in"), None);
    // `blur.enabled` is not `decoration:shadow:enabled`.
    let text = "decoration = {\n  blur = { enabled = true },\n  shadow = {\n    enabled = false,\n  },\n}\n";
    assert_eq!(find_in_user_lua(text, "decoration:shadow:enabled"), Some(4));
    assert_eq!(find_in_user_lua(text, "decoration:blur:enabled"), Some(2));
    assert_eq!(find_in_user_lua("gaps_in == 2", "general:gaps_in"), None);
}

#[test]
fn commented_lines_are_ignored() {
    let text = "-- general.gaps_in = 3 (old)\n-- hl.config({ general = { gaps_in = 1 } })\n\
        general = { -- gaps_in = 4\n  border_size = 1, -- general:gaps_in\n  gaps_in = 2,\n}\n";
    assert_eq!(find_in_user_lua(text, "general:gaps_in"), Some(5));
    let only = "--decoration = { blur = { enabled = false } }\n";
    assert_eq!(find_in_user_lua(only, "decoration:blur:enabled"), None);
}

#[test]
fn huge_and_non_utf8_input_is_safe() {
    // 1M nested braces open and close inside `general`, then `gaps_in` sits outside it.
    let bytes: Vec<u8> = b"general = {\n\xff\xfe gaps\xc3 = 1\n"
        .iter()
        .copied()
        .chain(std::iter::repeat_n(b'{', 1 << 20))
        .chain(std::iter::repeat_n(b'}', 1 << 20))
        .chain(b"}\n  gaps_in = 2\n".iter().copied())
        .collect();
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(find_in_user_lua(&text, "general:gaps_in"), None);
    let more = format!("{text}general = {{ gaps_in = 1 }}\n");
    assert_eq!(find_in_user_lua(&more, "general:gaps_in"), Some(5));
    let wide = "é".repeat(1 << 20) + "general:gaps_in";
    assert_eq!(find_in_user_lua(&wide, "general:gaps_in"), Some(1));
    assert_eq!(find_in_user_lua("anything", "nocolon"), None);
}

#[test]
fn every_option_key_exists_in_state_json() {
    let mut st = State::default();
    st.set_section(
        "look",
        json!({"gaps_in": 1, "gaps_out": 1, "border_width": 1, "rounding": 1, "blur": true,
            "blur_size": 2, "blur_passes": 2, "shadow": true, "animations": true,
            "animation_speed": 1.0}),
    )
    .unwrap();
    st.set_section(
        "layouts",
        json!({"default": "master", "dwindle": {"preserve_split": true, "smart_split": true,
            "force_split": "left"}, "master": {"mfact": 0.5, "new_status": "master",
            "orientation": "top"}, "scrolling": {"column_width": 0.5,
            "fullscreen_on_one_column": true}}),
    )
    .unwrap();
    st.set_section(
        "appearance",
        json!({"ui_font": "A", "mono_font": "B", "font_size": 10, "cursor_theme": "C",
            "cursor_size": 24, "icon_theme": "D"}),
    )
    .unwrap();
    st.set_section("compat", json!({"sharp_x11": true}))
        .unwrap();
    st.set_section(
        "input",
        json!({"swipe": {"distance": 400, "invert": false, "create_new": false,
            "cancel_ratio": 0.4},
            "keyboard": {"layouts": [{"layout": "us"}], "switch_combo": "grp:alt_shift_toggle",
                "repeat_rate": 30, "repeat_delay": 400},
            "touchpad": {"natural_scroll": true, "tap_to_click": true,
                "disable_while_typing": true, "scroll_factor": 1.5}}),
    )
    .unwrap();
    let mut keys = std::collections::HashSet::new();
    for row in OPTIONS {
        assert!(keys.insert(row.key), "duplicate {}", row.key);
        let section = st.section(row.domain()).unwrap();
        assert!(
            !row.stored(&section).is_null(),
            "{} not in {section}",
            row.key
        );
        assert_eq!(
            row.hypr.is_some() as u8 + row.gsettings.is_some() as u8,
            u8::from(row.key != "look.animation_speed"),
            "{} reads back from exactly one place",
            row.key
        );
        assert!(!row.setting_id.is_empty() && !row.label.is_empty());
    }
    for f in crate::settings::Look::FIELDS {
        assert!(keys.contains(format!("look.{f}").as_str()), "look.{f}");
    }
}

#[test]
fn labels_name_rows() {
    assert_eq!(label_for("look", "gaps_in"), Some("Gaps between windows"));
    assert_eq!(
        label_for("layouts", "dwindle.preserve_split"),
        Some("Dwindle: keep the split")
    );
    assert_eq!(label_for("look", "nope"), None);
    assert_eq!(label_for("wallpaper", "gaps_in"), None);
    let line = crate::history::summarize("look", &json!({}), &json!({"gaps_in": 3}), &|d, k| {
        label_for(d, k).map(Into::into)
    });
    assert!(
        line.starts_with("Gaps between windows: default → 3"),
        "{line}"
    );
}

#[test]
fn live_values_compare_across_forms() {
    assert!(live_matches(&json!(5), &json!("5 5 5 5")));
    assert!(!live_matches(&json!(5), &json!("5 2 5 5")));
    assert!(!live_matches(&json!(5), &json!("")));
    assert!(live_matches(&json!(5), &json!(5)));
    assert!(live_matches(&json!(0.55), &json!(0.550000)));
    assert!(live_matches(&json!(true), &json!(1)));
    assert!(live_matches(&json!(false), &json!(false)));
    assert!(!live_matches(&json!(true), &json!(0)));
    assert!(live_matches(&json!("dwindle"), &json!("dwindle")));
    assert!(live_matches(&json!(24), &json!("24")));
    assert!(!live_matches(&json!("Inter 11"), &json!("Inter 12")));
}

#[test]
fn expected_uses_overrides_then_theme() {
    let theme = Theme::from_toml(
        crate::theme::tests::FIXTURE,
        "fixture",
        crate::theme::Variant::Dark,
    )
    .unwrap();
    let mut st = State::default();
    let row = |k: &str| OPTIONS.iter().find(|r| r.key == k).unwrap();
    assert_eq!(expected(row("look.gaps_in"), &st, &theme), Some(json!(5)));
    assert_eq!(expected(row("look.blur"), &st, &theme), None);
    assert_eq!(
        expected(row("appearance.ui_font"), &st, &theme),
        Some(json!("Inter 11"))
    );
    st.set_section("look", json!({"gaps_in": 9, "blur": false}))
        .unwrap();
    st.set_section("appearance", json!({"font_size": 14}))
        .unwrap();
    st.set_section("layouts", json!({"dwindle": {"force_split": "right"}}))
        .unwrap();
    assert_eq!(expected(row("look.gaps_in"), &st, &theme), Some(json!(9)));
    assert_eq!(expected(row("look.blur"), &st, &theme), Some(json!(false)));
    assert_eq!(
        expected(row("appearance.font_size"), &st, &theme),
        Some(json!("Inter 14"))
    );
    assert_eq!(
        expected(row("layouts.dwindle.force_split"), &st, &theme),
        Some(json!(2))
    );
    assert_eq!(expected(row("look.animations"), &st, &theme), None);
    st.settings.ui.reduce_motion = true;
    assert_eq!(
        expected(row("look.animations"), &st, &theme),
        Some(json!(false)),
        "reduce motion renders animations off"
    );
}

#[test]
fn compat_sharp_x11_row_expected() {
    let theme = Theme::from_toml(
        crate::theme::tests::FIXTURE,
        "fixture",
        crate::theme::Variant::Dark,
    )
    .unwrap();
    let row = OPTIONS
        .iter()
        .find(|r| r.key == "compat.sharp_x11")
        .expect("the compat.sharp_x11 row");
    assert_eq!(row.setting_id, "sharp-x11");
    assert_eq!(row.label, "Sharp X11 apps");
    assert_eq!(row.hypr, Some(crate::compat::hypr::XWAYLAND_ZERO_SCALING));
    assert_eq!(row.null_source, NullSource::Default);
    let mut st = State::default();
    // Off renders nothing, so there is nothing to read back (F6).
    assert_eq!(expected(row, &st, &theme), None);
    st.set_section("compat", json!({"sharp_x11": true}))
        .unwrap();
    assert_eq!(expected(row, &st, &theme), Some(json!(true)));
    assert_eq!(label_for("compat", "sharp_x11"), Some("Sharp X11 apps"));
}

fn combo(s: &str) -> crate::keybinds::Combo {
    crate::keybinds::combo::parse(s).unwrap()
}

#[test]
fn finds_bind_and_unbind_lines() {
    let q = combo("SUPER + Q");
    let text = "-- top\nhl.bind(\"SUPER + Q\", hl.dsp.window.close())\n";
    assert_eq!(find_bind_in_user_lua(text, q), Some(2));
    let text = "x = 1\n\nhl.unbind(\"WIN + q\")\n";
    assert_eq!(find_bind_in_user_lua(text, q), Some(3));
    let text = "hl.bind ( 'META+q', d )\n";
    assert_eq!(find_bind_in_user_lua(text, q), Some(1));
    // A commented-out call, another shortcut, and a key built with `..` never hit.
    assert_eq!(
        find_bind_in_user_lua("-- hl.unbind(\"SUPER + Q\")\n", q),
        None
    );
    assert_eq!(
        find_bind_in_user_lua("hl.bind(\"SUPER + W\", d)\n", q),
        None
    );
    assert_eq!(
        find_bind_in_user_lua("hl.bind(mainMod .. \" + Q\", d)\n", q),
        None
    );
    assert_eq!(
        find_bind_in_user_lua("my_hl.bind(\"SUPER + Q\")\n", q),
        None
    );
    // `all` overrides every shortcut, but only as an unbind.
    let text = "a\nhl.unbind(\"all\")\n";
    assert_eq!(find_bind_in_user_lua(text, q), Some(2));
    assert_eq!(find_bind_in_user_lua(text, combo("CTRL + F5")), Some(2));
    assert_eq!(find_bind_in_user_lua("hl.bind(\"all\", d)\n", q), None);
}

#[test]
fn marked_binds_read_label_or_line() {
    let text = "hl.bind(\"SUPER + T\", d) -- uq:bind Terminal two\n\
                hl.bind(\"SUPER + Y\", d)\n\
                hl.bind(\"WIN + U\", d)  --uq:bind\n\
                hl.bind(mainMod .. \" + I\", d) -- uq:bind  \n\
                -- hl.bind(\"SUPER + O\", d) -- uq:bind\n\
                x = 1 -- uq:bind\n\
                hl.bind(\"SUPER + P\", d) -- uq:bindery\n";
    let m = marked_binds(text);
    assert_eq!(m.len(), 3, "{m:?}");
    assert_eq!(
        m[0],
        MarkedBind {
            line: 1,
            combo: Some("SUPER + T".into()),
            text: "Terminal two".into()
        }
    );
    assert_eq!(m[1].line, 3);
    assert_eq!(m[1].combo.as_deref(), Some("SUPER + U"));
    assert!(
        m[1].text.starts_with("hl.bind(\"WIN + U\""),
        "{}",
        m[1].text
    );
    assert_eq!((m[2].line, m[2].combo.clone()), (4, None));
    // Control characters are spaces, and a long line is cut.
    let long = format!(
        "hl.bind(\"SUPER + T\", \u{1b}[31m{}) -- uq:bind\n",
        "x".repeat(300)
    );
    let m = marked_binds(&long);
    assert_eq!(m[0].text.chars().count(), 120);
    assert!(!m[0].text.contains('\u{1b}'));
    // At most 100.
    let many = "hl.bind(\"SUPER + T\", d) -- uq:bind\n".repeat(150);
    assert_eq!(marked_binds(&many).len(), 100);
}

#[test]
fn find_gesture_in_user_lua_skips_comments() {
    let text = "-- hl.gesture({ fingers = 3 })\nhl.config({})\n  hl.gesture ( { fingers = 4 })\n";
    assert_eq!(find_gesture_in_user_lua(text), Some(3));
    assert_eq!(find_gesture_in_user_lua("x = my_hl.gesture(1)\n"), None);
    assert_eq!(
        find_gesture_in_user_lua("-- only a comment: hl.gesture(\n"),
        None
    );
    assert_eq!(find_gesture_in_user_lua("local g = hl.gesture\n"), None);
    assert_eq!(find_gesture_in_user_lua(""), None);
    assert_eq!(
        find_gesture_in_user_lua("hl.config({})\nhl.gesture({ fingers = 3 }) -- mine\n"),
        Some(2)
    );
}

#[test]
fn swipe_rows_read_back_what_is_rendered() {
    let mut st = State::default();
    st.set_section(
        "input",
        json!({"swipe": {"distance": 400, "cancel_ratio": 0.4}}),
    )
    .unwrap();
    let theme = crate::theme::Theme::from_toml(
        crate::theme::tests::FIXTURE,
        "fixture",
        crate::theme::Variant::Dark,
    )
    .unwrap();
    let row = |k: &str| OPTIONS.iter().find(|r| r.key == k).unwrap();
    assert_eq!(
        expected(row("input.swipe.distance"), &st, &theme),
        Some(json!(400))
    );
    assert_eq!(expected(row("input.swipe.invert"), &st, &theme), None);
    assert_eq!(
        expected(row("input.swipe.cancel_ratio"), &st, &theme),
        Some(json!(0.4))
    );
    for (_, lua) in crate::gestures::hypr::SWIPE_OPTIONS {
        assert!(
            OPTIONS
                .iter()
                .any(|r| r.hypr == Some(format!("gestures:{lua}").as_str())),
            "{lua}"
        );
    }
}
