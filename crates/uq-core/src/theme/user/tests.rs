use super::*;
use crate::theme::tests::FIXTURE;
use std::fs;
use std::path::PathBuf;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn dark_text() -> String {
    FIXTURE.to_string()
}

/// Gruvbox light, renamed to match the dark fixture.
fn light_text() -> String {
    let t = fs::read_to_string(repo().join("themes/gruvbox/light.toml")).unwrap();
    t.replacen("name = \"Gruvbox\"", "name = \"Fixture\"", 1)
}

fn dark() -> Theme {
    Theme::from_toml(&dark_text(), "user-x", Variant::Dark).unwrap()
}

fn light() -> Theme {
    Theme::from_toml(&light_text(), "user-x", Variant::Light).unwrap()
}

fn put(root: &Path, id: &str, d: &str, l: &str) {
    let dir = root.join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("dark.toml"), d).unwrap();
    fs::write(dir.join("light.toml"), l).unwrap();
}

fn good(root: &Path, id: &str) {
    put(root, id, &dark_text(), &light_text());
}

const HOSTILE_NAMES: [&str; 11] = [
    "X\nshell /bin/sh -c evil",
    "X */ * { color: red } /*",
    "{{ 7*7 }}",
    "{% if 1 %}",
    "{# x",
    "X\u{202e}",
    "X\u{200b}",
    "X\r",
    " X",
    "",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
];

#[test]
fn ids_and_slugs() {
    for ok in ["user-ocean", "user-a1-b2", "user-a", "user-0"] {
        assert!(valid_user_id(ok), "{ok}");
    }
    let long = format!("user-{}", "a".repeat(41));
    assert_eq!(long.len(), 46);
    for bad in [
        "user-",
        "ocean",
        "user-Ocean",
        "user--x",
        "user-x-",
        "user-../x",
        "user-a/b",
        "user-a\0",
        long.as_str(),
    ] {
        assert!(!valid_user_id(bad), "{bad:?}");
    }
    assert!(valid_user_id(&format!("user-{}", "a".repeat(40))));
    assert!(valid_slug("a-b") && !valid_slug("") && !valid_slug("a b"));
    assert_eq!(slug_for("My Ocean!", &[]), "my-ocean");
    assert_eq!(slug_for("My Ocean", &["my-ocean".into()]), "my-ocean-2");
    assert_eq!(
        slug_for("My Ocean", &["my-ocean".into(), "my-ocean-2".into()]),
        "my-ocean-3"
    );
    assert_eq!(slug_for("ëë", &[]), "theme");
    assert_eq!(slug_for(&"a".repeat(60), &[]).len(), 40);
    let full = "a".repeat(40);
    let next = slug_for(&"a".repeat(60), &[full]);
    assert_eq!(next.len(), 40);
    assert!(next.ends_with("-2") && valid_slug(&next), "{next}");
    assert!(valid_slug(&slug_for(
        "a-b-c-d-e-f-g-h-i-j-k-l-m-n-o-p-q-r-s-t-u",
        &[]
    )));
}

#[test]
fn display_text_rules() {
    for bad in HOSTILE_NAMES {
        assert!(!valid_display_text(bad), "{bad:?}");
    }
    for ok in ["Ocean", "Café Noir", "Rosé–Pine"] {
        assert!(valid_display_text(ok), "{ok}");
    }
    assert!(valid_display_text(&"a".repeat(64)));
}

#[test]
fn check_variant_ranges() {
    type Set = fn(&mut Theme, u8);
    let cases: [(&str, u8, u8, Set); 6] = [
        ("corner radius", 0, 40, |t, n| t.ui.radius = n),
        ("gap between windows", 0, 50, |t, n| t.ui.gap_in = n),
        ("gap to the screen edge", 0, 100, |t, n| t.ui.gap_out = n),
        ("border width", 0, 20, |t, n| t.ui.border_width = n),
        ("font size", 6, 32, |t, n| t.fonts.size = n),
        ("cursor size", 16, 96, |t, n| t.icons.cursor_size = n),
    ];
    for (what, lo, hi, set) in cases {
        for n in [lo, hi] {
            let mut t = dark();
            set(&mut t, n);
            assert_eq!(check_variant(&t, None), Ok(()), "{what} {n}");
        }
        for n in [hi + 1].into_iter().chain((lo > 0).then(|| lo - 1)) {
            let mut t = dark();
            set(&mut t, n);
            let e = check_variant(&t, None).unwrap_err();
            assert_eq!(e, Problem::OutOfRange(what));
            assert!(e.to_string().contains(what), "{e}");
        }
    }
    let mut t = dark();
    t.fonts.ui = "x".repeat(101);
    assert_eq!(check_variant(&t, None), Err(Problem::Font));
    t.fonts.ui = "Inter\n".into();
    assert_eq!(check_variant(&t, None), Err(Problem::Font));
    let mut t = dark();
    t.icons.theme = "../x".into();
    assert_eq!(check_variant(&t, None), Err(Problem::IconName));
    let mut t = dark();
    t.icons.cursor_theme = String::new();
    assert_eq!(check_variant(&t, None), Err(Problem::IconName));
    let mut t = dark();
    t.name = "X */".into();
    assert_eq!(check_variant(&t, None), Err(Problem::Name));
    assert_eq!(check_variant(&dark(), Some("a\nb")), Err(Problem::Author));
    assert_eq!(check_variant(&dark(), Some("Sam")), Ok(()));
}

#[test]
fn check_pair_contrast_and_brightness() {
    assert_eq!(check_pair(&dark(), &light(), None), Ok(()));
    let mut d = dark();
    d.colors.text_muted = d.colors.surface;
    let e = check_pair(&d, &light(), None).unwrap_err();
    assert!(
        matches!(
            &e,
            Problem::HardToRead {
                variant: Variant::Dark,
                ..
            }
        ),
        "{e:?}"
    );
    assert!(e.to_string().contains("dark"), "{e}");
    // A light palette in the dark slot passes contrast but not the brightness check.
    let wrong = Theme::from_toml(&light_text(), "user-x", Variant::Dark).unwrap();
    assert_eq!(
        check_pair(&wrong, &light(), None),
        Err(Problem::WrongBrightness(Variant::Dark))
    );
    let mut l = light();
    l.name = "Other".into();
    assert_eq!(check_pair(&dark(), &l, None), Err(Problem::NamesDiffer));
}

#[test]
fn scan_reads_good_and_skips_bad() {
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    good(r, "user-good");
    fs::create_dir(r.join("user-onlydark")).unwrap();
    fs::write(r.join("user-onlydark/dark.toml"), dark_text()).unwrap();
    good(r, "ocean");
    put(r, "user-big", &" ".repeat(65_537), &light_text());
    put(
        r,
        "user-dim",
        &dark_text().replace("text_muted = \"#bdae93\"", "text_muted = \"#32302f\""),
        &light_text(),
    );
    fs::write(r.join("notes.txt"), "hi").unwrap();
    let s = scan(r);
    assert!(!s.root_is_link);
    assert_eq!(s.themes.len(), 1);
    assert_eq!(s.themes[0].dark.id, "user-good");
    assert_eq!(s.themes[0].light.id, "user-good");
    let got: Vec<_> = s
        .skipped
        .iter()
        .map(|k| (k.folder.as_deref().unwrap(), &k.problem))
        .collect();
    assert_eq!(got.len(), 4, "{got:?}");
    assert_eq!(got[0], ("ocean", &Problem::FolderName));
    assert_eq!(got[1], ("user-big", &Problem::Unreadable(Variant::Dark)));
    assert!(matches!(got[2], ("user-dim", Problem::HardToRead { .. })));
    assert_eq!(
        got[3],
        ("user-onlydark", &Problem::MissingVariant(Variant::Light))
    );
}

#[test]
fn scan_reads_the_author() {
    let root = tempfile::tempdir().unwrap();
    let with = |t: String| format!("author = \"Sam\"\n{t}");
    put(
        root.path(),
        "user-a",
        &with(dark_text()),
        &with(light_text()),
    );
    put(
        root.path(),
        "user-b",
        &with(dark_text()).replace("Sam", "Sam\\n"),
        &light_text(),
    );
    let s = scan(root.path());
    assert_eq!(s.themes.len(), 1);
    assert_eq!(s.themes[0].author.as_deref(), Some("Sam"));
    assert_eq!(s.skipped.len(), 1);
    assert_eq!(s.skipped[0].problem, Problem::Author);
}

#[test]
fn scan_skips_links_and_bad_names() {
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    good(r, "user-real");
    std::os::unix::fs::symlink(r.join("user-real"), r.join("user-folderlink")).unwrap();
    good(r, "user-filelink");
    fs::remove_file(r.join("user-filelink/light.toml")).unwrap();
    std::os::unix::fs::symlink(
        r.join("user-real/light.toml"),
        r.join("user-filelink/light.toml"),
    )
    .unwrap();
    // A pipe named dark.toml must be skipped, not waited on.
    fs::create_dir(r.join("user-pipe")).unwrap();
    let ok = std::process::Command::new("mkfifo")
        .arg(r.join("user-pipe/dark.toml"))
        .status()
        .unwrap()
        .success();
    assert!(ok, "mkfifo");
    fs::write(r.join("user-pipe/light.toml"), light_text()).unwrap();
    fs::create_dir(r.join("Bad Name!")).unwrap();

    let (tx, rx) = std::sync::mpsc::channel();
    let path = r.to_path_buf();
    std::thread::spawn(move || tx.send(scan(&path)).ok());
    let s = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the scan waited on a pipe");
    assert_eq!(s.themes.len(), 1);
    assert_eq!(s.themes[0].dark.id, "user-real");
    let by = |f: &str| {
        s.skipped
            .iter()
            .find(|k| k.folder.as_deref() == Some(f))
            .map(|k| k.problem.clone())
    };
    assert_eq!(by("user-folderlink"), Some(Problem::Link));
    assert_eq!(by("user-filelink"), Some(Problem::Link));
    assert_eq!(by("user-pipe"), Some(Problem::Unreadable(Variant::Dark)));
    let odd = s.skipped.iter().find(|k| k.folder.is_none()).unwrap();
    assert_eq!(odd.problem, Problem::FolderName);

    // A symlinked root reads nothing.
    let base = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(r, base.path().join("themes")).unwrap();
    let s = scan(&base.path().join("themes"));
    assert!(s.root_is_link);
    assert!(s.themes.is_empty() && s.skipped.is_empty());
    assert!(fingerprint(&base.path().join("themes")).is_empty());
    // An absent root is just empty.
    let s = scan(&base.path().join("nope"));
    assert!(!s.root_is_link && s.themes.is_empty());
}

#[test]
fn scan_caps_themes() {
    let root = tempfile::tempdir().unwrap();
    for i in 0..51 {
        good(root.path(), &format!("user-t{i:02}"));
    }
    let s = scan(root.path());
    assert_eq!(s.themes.len(), MAX_USER_THEMES);
    assert_eq!(s.skipped.len(), 1);
    assert_eq!(s.skipped[0].problem, Problem::TooMany);
}

#[test]
fn scan_looks_at_a_bounded_number_of_entries() {
    let root = tempfile::tempdir().unwrap();
    for i in 0..(MAX_SCANNED + 20) {
        fs::create_dir(root.path().join(format!("junk{i:03}"))).unwrap();
    }
    let s = scan(root.path());
    assert_eq!(s.skipped.len(), MAX_SCANNED);
}

#[test]
fn fingerprint_changes_only_with_files() {
    let root = tempfile::tempdir().unwrap();
    good(root.path(), "user-a");
    let before = fingerprint(root.path());
    assert_eq!(before.len(), 2);
    assert_eq!(fingerprint(root.path()), before);
    fs::write(root.path().join("stray.txt"), "x").unwrap();
    assert_eq!(fingerprint(root.path()), before);
    let f = fs::OpenOptions::new()
        .write(true)
        .open(root.path().join("user-a/dark.toml"))
        .unwrap();
    f.set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000))
        .unwrap();
    assert_ne!(fingerprint(root.path()), before);
}

#[test]
fn variant_toml_round_trips() {
    let reg = crate::theme::ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    assert_eq!(reg.ids().len(), 7);
    for t in reg.iter() {
        for author in [None, Some("Sam \"the\" Maker")] {
            let text = variant_toml(t, author);
            let (back, a) = Theme::from_toml_with_author(&text, &t.id, t.variant).unwrap();
            assert_eq!(&back, t, "{} {}", t.id, t.variant);
            assert_eq!(a.as_deref(), author);
        }
    }
    let mut odd = dark();
    odd.name = "a#b\"c\\d'e".into();
    let back = Theme::from_toml(&variant_toml(&odd, None), "user-x", Variant::Dark).unwrap();
    assert_eq!(back, odd);
}

#[test]
fn hostile_names_never_leave_template_comments() {
    let templates = repo().join("templates");
    let comment_files = |t: &Theme| {
        let out = crate::render::render(t, &templates, Some("KvGnome")).unwrap();
        out.0
            .into_iter()
            .filter(|(p, _)| p.extension().is_some_and(|e| e != "json"))
            .collect::<Vec<_>>()
    };
    let plain = comment_files(&dark());
    assert!(plain.len() >= 6);
    let edge = [
        "a".repeat(64),
        "a#b".into(),
        "a/*b".into(),
        "a\"b".into(),
        "a\\b".into(),
        "a'b".into(),
        "é".into(),
    ];
    for name in edge {
        assert!(valid_display_text(&name), "{name:?} should be accepted");
        let mut t = dark();
        t.name = name.clone();
        let out = comment_files(&t);
        assert_eq!(out.len(), plain.len());
        for ((path, text), (_, base)) in out.iter().zip(&plain) {
            assert_eq!(
                text.lines().count(),
                base.lines().count(),
                "{path:?} {name}"
            );
            if text.contains("Generated by uqd from") {
                let first = text
                    .lines()
                    .find(|l| l.contains("Generated by uqd from"))
                    .unwrap();
                assert!(first.contains(&name), "{path:?}: {first}");
            }
            if path.extension().is_some_and(|e| e == "css") {
                let masked = text.replace(&name, "NAME");
                assert_eq!(
                    masked.matches("/*").count(),
                    masked.matches("*/").count(),
                    "{path:?} {name}"
                );
            }
        }
    }
}

#[test]
fn messages_are_plain() {
    let v = Variant::Dark;
    let all = [
        Problem::FolderName,
        Problem::Link,
        Problem::NotAFolder,
        Problem::MissingVariant(v),
        Problem::Unreadable(v),
        Problem::BadFile(v),
        Problem::NamesDiffer,
        Problem::Name,
        Problem::Author,
        Problem::Font,
        Problem::IconName,
        Problem::OutOfRange("corner radius"),
        Problem::HardToRead {
            variant: v,
            text: "Muted text on panels is hard to read.".into(),
        },
        Problem::WrongBrightness(v),
        Problem::TooMany,
    ];
    for p in all {
        let m = p.to_string();
        for bad in ["toml", "TOML", "serde", "uqd", "{", "[", "ThemeError"] {
            assert!(!m.contains(bad), "{m:?} has {bad:?}");
        }
        assert!(
            m.ends_with('.') && m.chars().next().unwrap().is_uppercase(),
            "{m}"
        );
    }
}
