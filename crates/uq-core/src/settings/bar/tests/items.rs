//! Island sizes, fill, text styles and per-island options.

use super::*;

#[test]
fn sizes_and_defaults() {
    let parse = |v| serde_json::from_value::<BarItem>(v).unwrap();
    assert_eq!(parse(json!({"id": "clock", "size": "normal"})).size, None);
    assert_eq!(
        parse(json!({"id": "clock", "size": "icon"})).size,
        Some(Size::Icon)
    );
    assert_eq!(parse(json!({"id": "clock"})).size(), Size::Medium);
    assert_eq!(parse(json!({"id": "keyboard"})).size(), Size::Icon);
    assert_eq!(parse(json!({"id": "security"})).size(), Size::Icon);
    assert!(known_island("security"));
    assert_eq!(parse(json!({"id": "work"})).size(), Size::Large);
    assert!(serde_json::from_value::<BarItem>(json!({"id": "clock", "size": "huge"})).is_err());
    assert_eq!(
        [Size::Icon, Size::Small, Size::Medium, Size::Large].map(Size::units),
        [1, 2, 4, 6]
    );
    let v = serde_json::to_value(BarItem {
        id: "news".into(),
        size: Some(Size::Icon),
        units: None,
        text: None,
        fill: None,
    })
    .unwrap();
    assert_eq!(v, json!({"id": "news", "size": "icon"}));
    assert_eq!(island_name("system"), Some("App menu, workspaces and dock"));
}

#[test]
fn fill() {
    let b = BarLayout::from_value(&json!({"start": [
        {"id": "work"}, {"id": "clock", "fill": true}, {"id": "status", "fill": false}
    ]}))
    .unwrap();
    assert!(b.start[0].fills(), "Open windows fills by default");
    assert!(b.start[1].fills());
    assert!(!b.start[2].fills());
    let off = BarLayout::from_value(&json!({"start": [{"id": "work", "fill": false}]})).unwrap();
    assert!(!off.start[0].fills());
    assert!(
        BarLayout::from_value(&json!({"start": [{"id": "separator", "fill": true}]})).is_err(),
        "a separator doesn't fill"
    );
    assert!(BarLayout::from_value(&json!({"start": [{"id": "work", "fill": "yes"}]})).is_err());
    assert_eq!(
        serde_json::to_value(BarItem::new("clock")).unwrap(),
        json!({"id": "clock"}),
        "not set is not written"
    );
}

#[test]
fn text_style() {
    let ok = BarLayout::from_value(&json!({"start": [
        {"id": "clock", "text": {"font": "Iosevka", "bold": true, "caps": true, "size": 130}},
        {"id": "work", "text": {"font": "monospace", "italic": true}}
    ]}))
    .unwrap();
    let t = ok.start[0].text.clone().unwrap();
    assert_eq!(
        (t.font.as_deref(), t.bold, t.italic, t.caps, t.size),
        (Some("Iosevka"), true, false, true, 130)
    );
    assert_eq!(ok.start[1].text.as_ref().unwrap().size, 100);
    // Only what differs from the theme is written.
    assert_eq!(
        serde_json::to_value(&ok.start[1]).unwrap(),
        json!({"id": "work", "text": {"font": "monospace", "italic": true}})
    );
    for bad in [
        json!({"start": [{"id": "clock", "text": {"size": 135}}]}),
        json!({"start": [{"id": "clock", "text": {"size": 60}}]}),
        json!({"start": [{"id": "clock", "text": {"size": 160}}]}),
        json!({"start": [{"id": "clock", "text": {"font": ""}}]}),
        json!({"start": [{"id": "clock", "text": {"font": "A\nB"}}]}),
        json!({"start": [{"id": "clock", "text": {"font": "x".repeat(65)}}]}),
        json!({"start": [{"id": "clock", "text": {"bold": "yes"}}]}),
        json!({"start": [{"id": "clock", "text": {"colour": "red"}}]}),
        json!({"start": [{"id": "separator", "text": {"bold": true}}]}),
    ] {
        assert!(BarLayout::from_value(&bad).is_err(), "{bad}");
    }
    // Lenient load: repaired, or dropped when nothing is left.
    let set: Settings = serde_json::from_value(json!({"shell": {"bar": {"start": [
        {"id": "clock", "text": {"size": 134, "font": "\u{7}", "bold": "yes"}},
        {"id": "work", "text": {"size": 999, "italic": true}},
        {"id": "system", "text": {"size": 100}},
        {"id": "separator", "text": {"bold": true}}
    ]}}}))
    .unwrap();
    let b = set.shell.bar.unwrap();
    let t = b.start[0].text.clone().unwrap();
    assert_eq!((t.font, t.bold, t.size), (None, false, 130));
    assert_eq!(b.start[1].text.as_ref().unwrap().size, MAX_TEXT_SIZE);
    assert_eq!(
        b.start[2].text,
        Some(TextStyle::default()),
        "the theme's look, set on purpose, is kept: it opts out of the all-islands text"
    );
    assert_eq!(b.start[3].text, None);
    assert!(b.validate().is_ok());
}

#[test]
fn custom_size() {
    let ok = BarLayout::from_value(&json!({
        "start": [{"id": "work", "size": "custom", "units": 9}]
    }))
    .unwrap();
    assert_eq!(ok.start[0].size(), Size::Custom);
    assert_eq!(ok.start[0].width_units(), 9);
    assert_eq!(BarItem::new("work").width_units(), 6);
    for bad in [
        json!({"start": [{"id": "work", "size": "custom"}]}),
        json!({"start": [{"id": "work", "size": "custom", "units": 6}]}),
        json!({"start": [{"id": "work", "size": "custom", "units": 25}]}),
        json!({"start": [{"id": "work", "size": "large", "units": 9}]}),
        json!({"start": [{"id": "work", "units": 9}]}),
    ] {
        assert!(BarLayout::from_value(&bad).is_err(), "{bad}");
    }
    // Lenient load: clamped, filled in, or dropped.
    let set: Settings = serde_json::from_value(json!({"shell": {"bar": {"start": [
        {"id": "work", "size": "custom", "units": 99},
        {"id": "system", "size": "custom"},
        {"id": "clock", "size": "small", "units": 9},
        {"id": "separator", "size": "custom", "units": 8}
    ]}}}))
    .unwrap();
    let b = set.shell.bar.unwrap();
    assert_eq!(b.start[0].units, Some(MAX_CUSTOM));
    assert_eq!(b.start[1].units, Some(MIN_CUSTOM));
    assert_eq!(b.start[2].units, None);
    assert_eq!((b.start[3].size, b.start[3].units), (None, None));
    assert!(b.validate().is_ok());
    // The Size edit stores the width; a tier clears it.
    let mut l = ok.clone();
    l.apply(&BarEdit::Size {
        id: "work".into(),
        zone: "start".into(),
        index: 0,
        size: Size::Custom,
        units: Some(12),
    })
    .unwrap();
    assert_eq!(l.start[0].width_units(), 12);
    l.apply(&BarEdit::Size {
        id: "work".into(),
        zone: "start".into(),
        index: 0,
        size: Size::Large,
        units: None,
    })
    .unwrap();
    assert_eq!((l.start[0].size, l.start[0].units), (None, None));
    assert!(
        l.apply(&BarEdit::Size {
            id: "work".into(),
            zone: "start".into(),
            index: 0,
            size: Size::Custom,
            units: None,
        })
        .is_err()
    );
}

#[test]
fn work_sort_options() {
    let b =
        BarLayout::from_value(&json!({"options": {"work": {"sort": "usage", "descending": true}}}))
            .unwrap();
    assert_eq!(b.options.work.sort, WindowSort::Usage);
    assert!(b.options.work.descending);
    assert_eq!(BarOptions::default().work.sort, WindowSort::Screen);
    let s = BarLayout::from_value(&json!({"options": {"work": {"sort": "screen"}}})).unwrap();
    assert_eq!(s.options.work.sort, WindowSort::Screen);
    for bad in [
        json!({"options": {"work": {"sort": "random"}}}),
        json!({"options": {"work": {"descending": "yes"}}}),
        json!({"options": {"work": {"nope": 1}}}),
    ] {
        assert!(BarLayout::from_value(&bad).is_err(), "{bad}");
    }
    #[derive(Deserialize)]
    struct W {
        #[serde(deserialize_with = "lenient_bar")]
        bar: Option<BarLayout>,
    }
    let w: W =
        serde_json::from_value(json!({"bar": {"options": {"work": {"sort": "random"}}}})).unwrap();
    assert_eq!(w.bar.unwrap().options.work, WorkOptions::default());
    let mut b = BarLayout::default();
    b.apply(
        &serde_json::from_value(
            json!({"op": "options", "island": "work", "value": {"sort": "name"}}),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(b.options.work.sort, WindowSort::Name);
}

#[test]
fn work_close_button() {
    assert_eq!(WorkOptions::default().close, CloseButton::Ask);
    let b = BarLayout::from_value(&json!({"options": {"work": {"close": "minimize"}}})).unwrap();
    assert_eq!(b.options.work.close, CloseButton::Minimize);
    assert_eq!(
        b.options.work.sort,
        WindowSort::Screen,
        "the rest as default"
    );
    let b = BarLayout::from_value(&json!({"options": {"work": {"close": "close"}}})).unwrap();
    assert_eq!(b.options.work.close, CloseButton::Close);
    assert!(BarLayout::from_value(&json!({"options": {"work": {"close": "hide"}}})).is_err());
}

#[test]
fn all_islands_text_default() {
    let mut b = BarLayout::default();
    assert!(b.options.all.text.is_default());
    // The theme's text is not written out.
    assert!(
        serde_json::to_value(&b.options).unwrap()["all"]
            .as_object()
            .unwrap()
            .is_empty()
    );
    b.apply(
        &serde_json::from_value(json!({"op": "options", "island": "all",
            "value": {"text": {"bold": true, "size": 120, "font": "monospace"}}}))
        .unwrap(),
    )
    .unwrap();
    assert!(b.options.all.text.bold);
    assert_eq!(b.options.all.text.size, 120);
    assert!(b.validate().is_ok());
    // A text size off the steps, a bad font and unknown fields are refused.
    for bad in [
        json!({"text": {"size": 125}}),
        json!({"text": {"font": "  "}}),
        json!({"text": {"nope": 1}}),
        json!({"size": "large"}),
    ] {
        let r = b.clone().apply(
            &serde_json::from_value(json!({"op": "options", "island": "all", "value": bad}))
                .unwrap(),
        );
        assert!(r.is_err(), "{bad}");
    }
    let v =
        BarLayout::from_value(&json!({"options": {"all": {"text": {"italic": true}}}})).unwrap();
    assert!(v.options.all.text.italic);
    // Loading repairs a size off the steps instead of failing.
    #[derive(Deserialize)]
    struct W {
        #[serde(deserialize_with = "lenient_bar")]
        bar: Option<BarLayout>,
    }
    let w: W =
        serde_json::from_value(json!({"bar": {"options": {"all": {"text": {"size": 117}}}}}))
            .unwrap();
    assert_eq!(w.bar.unwrap().options.all.text.size, 120);
}
