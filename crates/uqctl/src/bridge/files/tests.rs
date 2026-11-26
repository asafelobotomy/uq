use super::*;
use std::fs;

fn tree() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    for dir in [
        "Documents/Taxes",
        "Pictures",
        "Music",
        "proj/src",
        "proj/target/debug",
        "proj/node_modules/x",
        ".hidden",
    ] {
        fs::create_dir_all(r.join(dir)).unwrap();
    }
    for f in [
        "Documents/Taxes/report-2026.pdf",
        "Documents/notes.md",
        "Pictures/report.png",
        "Music/song.flac",
        "proj/src/main.rs",
        "proj/target/debug/report.bin",
        "proj/node_modules/x/report.js",
        ".hidden/report.txt",
        "report",
        "archive.tar",
    ] {
        fs::write(r.join(f), "x").unwrap();
    }
    std::os::unix::fs::symlink(r.join("Documents"), r.join("docs-link")).unwrap();
    d
}

#[test]
fn kinds_from_extensions() {
    assert_eq!(Kind::of_file("a.PDF"), Kind::Document);
    assert_eq!(Kind::of_file("a.jpeg"), Kind::Image);
    assert_eq!(Kind::of_file("a.flac"), Kind::Audio);
    assert_eq!(Kind::of_file("a.mkv"), Kind::Video);
    assert_eq!(Kind::of_file("a.rs"), Kind::Code);
    assert_eq!(Kind::of_file("a.tar"), Kind::Archive);
    assert_eq!(Kind::of_file("Makefile"), Kind::Other);
    assert_eq!(Kind::of_file(".bashrc"), Kind::Other);
    assert_eq!(Kind::of_file("a.weird"), Kind::Other);
}

#[test]
fn index_skips_hidden_build_folders_and_links() {
    let d = tree();
    let idx = build(&[d.path().to_path_buf()], SystemTime::now());
    let paths: Vec<&str> = idx.entries.iter().map(|e| &*e.path).collect();
    assert!(paths.iter().any(|p| p.ends_with("report-2026.pdf")));
    assert!(paths.iter().any(|p| p.ends_with("/Taxes")));
    for gone in ["target", "node_modules", ".hidden"] {
        assert!(!paths.iter().any(|p| p.contains(gone)), "{gone}: {paths:?}");
    }
    assert!(!idx.truncated);
}

#[test]
fn index_follows_links_once_and_never_loops() {
    let d = tree();
    let idx = build(&[d.path().to_path_buf()], SystemTime::now());
    let paths: Vec<&str> = idx.entries.iter().map(|e| &*e.path).collect();
    // `docs-link` points at Documents, which is walked once, under whichever name comes first.
    assert_eq!(
        paths.iter().filter(|p| p.ends_with("notes.md")).count(),
        1,
        "{paths:?}"
    );
    // A link back to a parent doesn't loop.
    std::os::unix::fs::symlink(d.path(), d.path().join("Documents/up")).unwrap();
    let idx = build(&[d.path().to_path_buf()], SystemTime::now());
    assert!(!idx.truncated);
}

/// The bug behind "I searched for a document and got nothing": `~/Documents` is a link to
/// another drive, and links were skipped.
#[test]
fn documents_folder_that_is_a_link_is_searched() {
    let home = tempfile::tempdir().unwrap();
    let drive = tempfile::tempdir().unwrap();
    fs::create_dir_all(drive.path().join("Documents/Taxes")).unwrap();
    fs::write(
        drive.path().join("Documents/Taxes/tax-return-2026.pdf"),
        "x",
    )
    .unwrap();
    fs::write(drive.path().join("Documents/cv.odt"), "x").unwrap();
    std::os::unix::fs::symlink(
        drive.path().join("Documents"),
        home.path().join("Documents"),
    )
    .unwrap();
    // A link to a single file, and a dangling one.
    std::os::unix::fs::symlink(
        drive.path().join("Documents/cv.odt"),
        home.path().join("cv-link.odt"),
    )
    .unwrap();
    std::os::unix::fs::symlink(home.path().join("nowhere"), home.path().join("gone.pdf")).unwrap();
    let idx = build(&[home.path().to_path_buf()], SystemTime::now());
    let found = search(&idx, "tax-return");
    assert_eq!(found[0].0, Kind::Document);
    assert_eq!(found[0].1[0].title, "tax-return-2026.pdf");
    assert!(found[0].1[0].subtitle.ends_with("/Documents/Taxes"));
    let docs = search(&idx, "cv");
    assert_eq!(docs[0].1.len(), 2, "{docs:?}");
    assert!(search(&idx, "gone").is_empty());
}

#[test]
fn shallow_files_survive_truncation() {
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("important.pdf"), "x").unwrap();
    let deep = d.path().join("a/b/c");
    fs::create_dir_all(&deep).unwrap();
    for i in 0..50 {
        fs::write(deep.join(format!("junk{i}.txt")), "x").unwrap();
    }
    // Breadth-first: with room for only a few entries the top level still comes first.
    let top: Vec<_> = build(&[d.path().to_path_buf()], SystemTime::now())
        .entries
        .iter()
        .take(3)
        .map(|e| e.name.to_string())
        .collect();
    assert!(top.contains(&"important.pdf".to_string()), "{top:?}");
}

#[test]
fn several_words_match_across_separators_in_any_order() {
    let d = tempfile::tempdir().unwrap();
    for f in ["Tax_Return-2026.pdf", "tax notes.md", "return.txt"] {
        fs::write(d.path().join(f), "x").unwrap();
    }
    let idx = build(&[d.path().to_path_buf()], SystemTime::now());
    let titles = |q: &str| -> Vec<String> {
        search(&idx, q)
            .into_iter()
            .flat_map(|(_, r)| r)
            .map(|r| r.title)
            .collect()
    };
    assert_eq!(titles("tax return"), vec!["Tax_Return-2026.pdf"]);
    assert_eq!(titles("return tax"), vec!["Tax_Return-2026.pdf"]);
    assert_eq!(titles("  tax   notes "), vec!["tax notes.md"]);
    assert!(titles("tax nothing").is_empty());
}

#[test]
fn search_groups_by_type_best_first() {
    let d = tree();
    let idx = build(&[d.path().to_path_buf()], SystemTime::now());
    let found = search(&idx, "Report");
    let kinds: Vec<Kind> = found.iter().map(|(k, _)| *k).collect();
    assert_eq!(kinds, vec![Kind::Document, Kind::Image, Kind::Other]);
    let other = &found[2].1;
    assert_eq!(other[0].title, "report");
    assert_eq!(found[1].1[0].title, "report.png");
    assert!(
        found
            .iter()
            .flat_map(|(_, r)| r)
            .all(|r| r.score <= 0.8 + 1e-9)
    );
    assert_eq!(search(&idx, "taxes")[0].0, Kind::Folder);
    assert!(search(&idx, "r").is_empty(), "one letter is too short");
    assert!(search(&idx, "zzzz").is_empty());
}

#[test]
fn whole_names_rank_above_parts() {
    assert!(name_score("report", "report").unwrap() > name_score("report", "report.png").unwrap());
    assert!(
        name_score("report", "report.png").unwrap() > name_score("report", "reports.txt").unwrap()
    );
    assert!(name_score("tax", "my-taxes").unwrap() > name_score("tax", "syntax").unwrap());
    assert_eq!(name_score("zz", "abc"), None);
}

#[test]
fn per_kind_is_capped() {
    let d = tempfile::tempdir().unwrap();
    for i in 0..12 {
        fs::write(d.path().join(format!("img{i}.png")), "x").unwrap();
    }
    let idx = build(&[d.path().to_path_buf()], SystemTime::now());
    let found = search(&idx, "img");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1.len(), PER_KIND);
}

#[test]
fn rows_show_home_as_tilde() {
    let home = std::env::var("HOME").unwrap();
    let r = row(Kind::Document, &format!("{home}/Documents/a.pdf"), 0.5);
    assert_eq!(r.title, "a.pdf");
    assert_eq!(r.subtitle, "~/Documents");
    assert_eq!(r.icon, "x-office-document");
}

/// Times a build and searches over the real home folder and drives:
/// `cargo test -p uqctl --release -- --ignored files::tests::timing --nocapture`.
#[test]
#[ignore = "reads the real home folder"]
fn timing() {
    let roots = default_roots();
    let t = Instant::now();
    let idx = build(&roots, SystemTime::now());
    println!(
        "indexed {} entries in {:?} (truncated: {})",
        idx.entries.len(),
        t.elapsed(),
        idx.truncated
    );
    for q in ["re", "report", "screenshot", "main.rs", "zzzzqq"] {
        let t = Instant::now();
        let n: usize = search(&idx, q).iter().map(|(_, r)| r.len()).sum();
        println!("search {q:?}: {n} rows in {:?}", t.elapsed());
    }
}

#[test]
fn stale_after_five_minutes() {
    let mut f = Files::default();
    let t = Instant::now();
    assert!(f.wants_build(t));
    f.built = Some(t);
    assert!(!f.wants_build(t + Duration::from_secs(60)));
    assert!(f.wants_build(t + STALE_AFTER));
    f.building = true;
    assert!(!f.wants_build(t + STALE_AFTER));
}
