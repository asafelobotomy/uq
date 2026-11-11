use super::*;
use crate::pages::fake_system::{FakeLocale, SysRig};
use crate::pages::runner::run;

fn fake_run(program: &str, args: &[&str]) -> Option<String> {
    match (program, args.first().copied()) {
        ("localectl", Some("list-locales")) => {
            Some("C.UTF-8\nde_DE.UTF-8\nen_GB.UTF-8\nen_US.UTF-8\n".into())
        }
        ("localectl", Some("list-keymaps")) => Some("de\nuk\nus\n".into()),
        _ => None,
    }
}

fn no_commands(_: &str, _: &[&str]) -> Option<String> {
    None
}

async fn setup() -> (SysRig, FakeLocale, RegionState) {
    let mut rig = SysRig::new();
    let fake = FakeLocale::start(&mut rig).await;
    let page = RegionState::with(rig.system(), PathBuf::from("/nonexistent"), fake_run);
    (rig, fake, page)
}

fn vars(fake: &FakeLocale) -> Vec<String> {
    let mut v = fake.state.lock().unwrap().locale.clone();
    v.sort();
    v
}

#[tokio::test]
async fn reads_language_formats_keymap_and_lists() {
    let (_rig, _fake, page) = setup().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let l = &out.state["localed"];
    assert_eq!(l["language"], "en_GB.UTF-8");
    assert_eq!(l["formats"], "en_GB.UTF-8", "formats follow the language");
    assert_eq!(l["keymap"], "uk");
    assert_eq!(out.state["locales"].as_array().unwrap().len(), 4);
    assert_eq!(out.state["keymaps"], json!(["de", "uk", "us"]));
}

#[tokio::test]
async fn formats_language_and_keymap_round_trip() {
    let (rig, fake, page) = setup().await;
    let out = run(
        &page,
        Op::SetFormats {
            locale: "de_DE.UTF-8".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["localed"]["formats"], "de_DE.UTF-8");
    assert_eq!(out.state["localed"]["language"], "en_GB.UTF-8");
    assert_eq!(out.state["notice"]["text"], LOCALE_NOTE);
    assert_eq!(vars(&fake).len(), 6, "LANG and the five format variables");

    let out = run(
        &page,
        Op::SetLanguage {
            locale: "en_US.UTF-8".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["localed"]["language"], "en_US.UTF-8");
    assert_eq!(
        out.state["localed"]["formats"], "de_DE.UTF-8",
        "set on their own: kept"
    );

    // Formats back to the language: the variables go, so they follow it again.
    let out = run(
        &page,
        Op::SetFormats {
            locale: "en_US.UTF-8".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(vars(&fake), ["LANG=en_US.UTF-8"]);

    let out = run(
        &page,
        Op::SetKeymap {
            keymap: "de".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["localed"]["keymap"], "de");

    let again = RegionState::with(rig.system(), PathBuf::from("/nonexistent"), fake_run);
    let out = run(&again, Op::Refresh).await;
    assert_eq!(out.state["localed"]["language"], "en_US.UTF-8");
    assert_eq!(out.state["localed"]["keymap"], "de");
}

#[tokio::test]
async fn refusals_and_unknown_values() {
    let (_rig, fake, page) = setup().await;
    let out = run(
        &page,
        Op::SetLanguage {
            locale: "xx_XX.UTF-8".into(),
        },
    )
    .await;
    assert!(out.error.unwrap().contains("isn't set up on this computer"));
    let out = run(
        &page,
        Op::SetKeymap {
            keymap: "dvorak-xx".into(),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("That keyboard layout isn't installed.")
    );
    fake.deny.set(true);
    let out = run(
        &page,
        Op::SetLanguage {
            locale: "de_DE.UTF-8".into(),
        },
    )
    .await;
    assert_eq!(out.error, None, "quiet");
    assert_eq!(out.state["localed"]["language"], "en_GB.UTF-8");
    assert_eq!(out.state["notice"]["text"], "");
}

#[tokio::test]
async fn lists_fall_back_to_files_and_service_can_be_missing() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("etc")).unwrap();
    std::fs::write(
        d.path().join("etc/locale.gen"),
        "#  de_DE.UTF-8 UTF-8\nen_GB.UTF-8 UTF-8\n\nfr_FR.UTF-8 UTF-8\n",
    )
    .unwrap();
    let km = d.path().join("usr/share/kbd/keymaps/i386/qwerty");
    std::fs::create_dir_all(&km).unwrap();
    std::fs::write(km.join("us.map.gz"), "").unwrap();
    std::fs::write(km.join("uk.map.gz"), "").unwrap();
    let rig = SysRig::new();
    let page = RegionState::with(rig.system(), d.path().to_path_buf(), no_commands);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["locales"], json!(["en_GB.UTF-8", "fr_FR.UTF-8"]));
    assert_eq!(out.state["keymaps"], json!(["uk", "us"]));
    assert_eq!(out.state["localed"]["available"], false);
    assert_eq!(out.state["localed"]["package"], "systemd");
    let out = run(
        &page,
        Op::SetLanguage {
            locale: "fr_FR.UTF-8".into(),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("The language and keyboard service isn't available. Install the systemd package.")
    );
}

#[test]
fn variable_edits() {
    let v = vec![
        "LANG=en_GB.UTF-8".to_string(),
        "LC_TIME=de_DE.UTF-8".to_string(),
    ];
    assert_eq!(get(&v, "LC_TIME"), Some("de_DE.UTF-8"));
    assert_eq!(get(&v, "LC_TIM"), None);
    assert_eq!(with_var(&v, "LC_TIME", None), ["LANG=en_GB.UTF-8"]);
    assert_eq!(
        with_var(&v, "LANG", Some("C.UTF-8")),
        ["LC_TIME=de_DE.UTF-8", "LANG=C.UTF-8"]
    );
}
