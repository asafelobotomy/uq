use super::fill::{fill_native, parse_done, write_done};
use super::info::{comment_for_test, describe_scheme, scheme};
use super::*;
use crate::mimeapps::default_for;

fn h(id: &str, mimes: &[&str]) -> Handler {
    Handler {
        id: id.into(),
        name: id.into(),
        icon: String::new(),
        mimes: mimes.iter().map(|m| m.to_string()).collect(),
    }
}

#[test]
fn a_family_kind_gives_every_type_of_it_the_app_opens() {
    let reelo = h(
        REELO,
        &["video/mp4", "video/ogg", "audio/flac", "video/x-flv"],
    );
    let video = kind("video").unwrap();
    assert_eq!(
        video.mimes_for(&reelo),
        ["video/mp4", "video/ogg", "video/x-flv"]
    );
    let music = kind("music").unwrap();
    assert_eq!(music.mimes_for(&reelo), ["audio/flac"]);
    // The browser has no family: only its own types.
    let ff = h(
        "firefox.desktop",
        &["text/html", "x-scheme-handler/http", "text/xml"],
    );
    assert_eq!(
        kind("browser").unwrap().mimes_for(&ff),
        ["x-scheme-handler/http", "text/html"]
    );
    assert!(
        kind("images")
            .unwrap()
            .mimes_to_clear(&[h("x", &["image/avif"])])
            .contains(&"image/avif".to_string())
    );
}

#[test]
fn removes_only_the_named_keys() {
    let text = "[Added Associations]\nimage/png=a.desktop;\n[Default Applications]\nimage/png=b.desktop;\n# keep\ntext/plain=c.desktop;\n";
    let out = remove_defaults(text, &["image/png"]).unwrap();
    assert_eq!(
        out,
        "[Added Associations]\nimage/png=a.desktop;\n[Default Applications]\n# keep\ntext/plain=c.desktop;\n"
    );
    assert_eq!(remove_defaults(&out, &["image/png"]), None);
}

#[test]
fn fills_only_kinds_never_chosen_and_only_once() {
    let apps = vec![
        h(UQVIEW, &["image/png", "image/avif"]),
        h(REELO, &["video/mp4", "audio/mpeg"]),
    ];
    let user = "[Default Applications]\naudio/mpeg=mpv.desktop;\n";
    let f = fill_native(user, &[], &apps, &[]);
    let text = f.text.clone().unwrap();
    assert_eq!(default_for(&text, "image/avif").as_deref(), Some(UQVIEW));
    assert_eq!(default_for(&text, "video/mp4").as_deref(), Some(REELO));
    assert_eq!(
        default_for(&text, "audio/mpeg").as_deref(),
        Some("mpv.desktop"),
        "a choice of their own stays"
    );
    assert_eq!(
        f.done,
        ["images", "video", "music"],
        "uQbit isn't installed"
    );
    assert_eq!(
        f.set,
        [("images", UQVIEW.to_string()), ("video", REELO.to_string())]
    );

    // Next login, after a reset of pictures: nothing comes back.
    let reset = remove_defaults(&text, &["image/png", "image/avif"]).unwrap();
    let again = fill_native(&reset, &[], &apps, &f.done);
    assert_eq!(again.text, None);
    assert_eq!(parse_done(&write_done(&again.done)), again.done);

    // A choice in the desktop's own list counts as theirs.
    let ov = vec!["[Default Applications]\nimage/png=feh.desktop;\n".to_string()];
    assert!(
        fill_native("", &ov, &apps, &[])
            .set
            .iter()
            .all(|(k, _)| *k != "images")
    );
}

#[test]
fn names_for_types_and_links() {
    assert_eq!(
        comment_for_test(
            "<mime-type><comment>PNG &amp; image</comment><comment xml:lang=\"de\">x</comment>"
        )
        .as_deref(),
        Some("PNG & image")
    );
    assert_eq!(scheme("x-scheme-handler/magnet"), Some("magnet"));
    assert_eq!(scheme("image/png"), None);
    assert_eq!(describe_scheme("magnet"), "Torrent magnet links");
    assert_eq!(describe_scheme("foo"), "foo: links");
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("image")).unwrap();
    std::fs::write(
        d.path().join("image/png.xml"),
        "<comment>PNG image</comment>",
    )
    .unwrap();
    let dirs = vec![d.path().to_path_buf()];
    assert_eq!(
        info::describe(&dirs, "image/png").as_deref(),
        Some("PNG image")
    );
    assert_eq!(info::describe(&dirs, "image/../png"), None);
    assert_eq!(info::describe(&dirs, "image/jpeg"), None);
}
