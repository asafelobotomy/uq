use super::*;
use crate::keybinds::defaults::TERMINAL;

#[test]
fn terminals_are_the_ones_the_shortcut_runs_in_its_order() {
    let [first, second] = TERMINAL_APPS;
    assert!(
        TERMINAL.ends_with(&format!(
            "'command -v {first} >/dev/null && exec {first}; exec {second}'"
        )),
        "{TERMINAL}"
    );
}

#[test]
fn creates_the_group_in_an_empty_file() {
    assert_eq!(
        with_terminal("", "kitty").unwrap(),
        "[General]\nTerminalApplication=kitty\n"
    );
}

#[test]
fn adds_the_key_to_an_existing_general_group() {
    let old = "[General]\nfixed=JetBrains Mono,10\n\n[Icons]\nTheme=Papirus-Dark\n";
    assert_eq!(
        with_terminal(old, "kitty").unwrap(),
        "[General]\nTerminalApplication=kitty\nfixed=JetBrains Mono,10\n\n[Icons]\nTheme=Papirus-Dark\n"
    );
}

#[test]
fn puts_a_new_general_group_first_and_keeps_the_rest() {
    let old = "[KDE]\nSingleClick=false\n";
    assert_eq!(
        with_terminal(old, "kitty").unwrap(),
        "[General]\nTerminalApplication=kitty\n\n[KDE]\nSingleClick=false\n"
    );
}

#[test]
fn keeps_a_terminal_the_user_chose() {
    for theirs in [
        "[General]\nTerminalApplication=konsole\n",
        "[General]\nTerminalApplication = alacritty\n",
        "[General]\nTerminalApplication[$e]=$HOME/bin/term\n",
        "[General]\nTerminalApplication=kitty\n",
    ] {
        assert_eq!(with_terminal(theirs, "kitty"), None, "{theirs:?}");
    }
}

#[test]
fn fills_an_empty_value() {
    let old = "[General]\nTerminalApplication=\nfixed=Mono,10\n";
    assert_eq!(
        with_terminal(old, "kitty").unwrap(),
        "[General]\nTerminalApplication=kitty\nfixed=Mono,10\n"
    );
}

#[test]
fn the_key_in_another_group_does_not_count() {
    let old = "[General]\nfixed=Mono,10\n\n[Other]\nTerminalApplication=konsole\n";
    let new = with_terminal(old, "kitty").unwrap();
    assert_eq!(
        new,
        "[General]\nTerminalApplication=kitty\nfixed=Mono,10\n\n[Other]\nTerminalApplication=konsole\n"
    );
    assert_eq!(with_terminal(&new, "kitty"), None, "second run: unchanged");
}

#[test]
fn writes_once_and_then_leaves_the_file_alone() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("kdeglobals");
    assert!(ensure_terminal_at(&path, "kitty").unwrap(), "created");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[General]\nTerminalApplication=kitty\n"
    );
    assert!(!ensure_terminal_at(&path, "kitty").unwrap(), "unchanged");
    std::fs::write(&path, "[General]\nTerminalApplication=konsole\n").unwrap();
    assert!(!ensure_terminal_at(&path, "kitty").unwrap());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[General]\nTerminalApplication=konsole\n"
    );
}

#[test]
fn conf_path_is_in_the_config_dir() {
    let p = Paths::under(Path::new("/h"));
    assert_eq!(
        conf_path(&p),
        p.hypr_dir.parent().unwrap().join("kdeglobals")
    );
}

const SCHEME_TXT: &str = "[General]\nColorScheme=uQ\nName=uQ Gruvbox\n\n[Colors:Window]\nBackgroundNormal=40,40,40\n\n[Colors:View]\nBackgroundNormal=60,56,54\n\n[WM]\nactiveBackground=1,2,3\n";

#[test]
fn colours_go_into_an_empty_file_without_the_wm_group() {
    let new = with_colors("", SCHEME_TXT).unwrap();
    assert_eq!(
        new,
        "[General]\nColorScheme=uQ\n\n[Colors:Window]\nBackgroundNormal=40,40,40\n\n[Colors:View]\nBackgroundNormal=60,56,54\n"
    );
    assert_eq!(with_colors(&new, SCHEME_TXT), None, "second run: unchanged");
}

#[test]
fn colours_keep_the_rest_and_the_terminal() {
    let old = "[General]\nTerminalApplication=kitty\n\n[Icons]\nTheme=Papirus-Dark\n";
    let new = with_colors(old, SCHEME_TXT).unwrap();
    assert!(
        new.starts_with("[General]\nColorScheme=uQ\nTerminalApplication=kitty\n"),
        "{new}"
    );
    assert!(new.contains("[Icons]\nTheme=Papirus-Dark\n"), "{new}");
    assert!(new.contains("[Colors:Window]"), "{new}");
}

#[test]
fn colours_replace_an_older_uq_scheme() {
    let old = with_colors("", SCHEME_TXT).unwrap();
    let next = SCHEME_TXT.replace("40,40,40", "9,9,9");
    let new = with_colors(&old, &next).unwrap();
    assert!(new.contains("BackgroundNormal=9,9,9"), "{new}");
    assert!(!new.contains("40,40,40"), "{new}");
    assert_eq!(new.matches("[Colors:Window]").count(), 1);
}

#[test]
fn colours_leave_a_users_own_scheme_alone() {
    let theirs = "[General]\nColorScheme=BreezeDark\n\n[Colors:Window]\nBackgroundNormal=1,1,1\n";
    assert_eq!(with_colors(theirs, SCHEME_TXT), None);
}

const SCHEME: &str = "[General]\nColorScheme=uQ\n\n[Colors:Window]\nBackgroundNormal=1,2,3\n";

fn look(icons: &str) -> Look<'_> {
    Look {
        icons,
        ui_font: "Inter",
        mono_font: "JetBrains Mono",
        size: 11,
        style: "kvantum",
    }
}

#[test]
fn look_sets_icons_style_and_fonts_and_keeps_the_rest() {
    let new = with_look(
        "[General]\nTerminalApplication=kitty\n",
        SCHEME,
        &look("Papirus-Dark"),
        false,
    )
    .unwrap();
    assert!(new.contains("TerminalApplication=kitty"), "{new}");
    assert!(new.contains("[Icons]\nTheme=Papirus-Dark"), "{new}");
    assert!(new.contains("[KDE]\nwidgetStyle=kvantum"), "{new}");
    assert!(
        new.contains("font=Inter,11,-1,5,400,0,0,0,0,0,0,0,0,0,0,1"),
        "{new}"
    );
    assert!(
        new.contains("fixed=JetBrains Mono,11,-1,5,400,0,0,0,1,"),
        "{new}"
    );
    assert!(new.contains("[Colors:Window]"), "{new}");
    assert_eq!(
        with_look(&new, SCHEME, &look("Papirus-Dark"), false),
        None,
        "second pass is a no-op: {new:?}"
    );
}

#[test]
fn look_changes_with_the_theme() {
    let a = with_look("", SCHEME, &look("Papirus-Dark"), false).unwrap();
    let b = with_look(&a, SCHEME, &look("Papirus-Light"), false).unwrap();
    assert!(
        b.contains("Theme=Papirus-Light") && !b.contains("Papirus-Dark"),
        "{b}"
    );
}

#[test]
fn look_leaves_a_users_own_scheme_alone() {
    let theirs = "[General]\nColorScheme=BreezeDark\n\n[Colors:Window]\nBackgroundNormal=9,9,9\n";
    assert_eq!(
        with_look(theirs, SCHEME, &look("Papirus-Dark"), false),
        None
    );
}

#[test]
fn reclaim_replaces_another_scheme_and_keeps_the_rest() {
    let theirs = "[General]\nColorScheme=BreezeDark\nTerminalApplication=kitty\n\n[Colors:Window]\nBackgroundNormal=9,9,9\n\n[KDE]\nSingleClick=false\n";
    let new = with_look(theirs, SCHEME, &look("Papirus-Dark"), true).unwrap();
    assert!(
        !new.contains("9,9,9") && !new.contains("BreezeDark"),
        "{new}"
    );
    assert!(new.contains("ColorScheme=uQ"), "{new}");
    assert!(new.contains("TerminalApplication=kitty"), "{new}");
    assert!(new.contains("SingleClick=false"), "{new}");
    assert!(new.contains("widgetStyle=kvantum"), "{new}");
    assert_eq!(
        with_look(&new, SCHEME, &look("Papirus-Dark"), true),
        None,
        "second pass"
    );
    assert!(has_foreign_colors(theirs) && !has_foreign_colors(&new));
}

#[test]
fn sync_look_copies_another_scheme_before_reclaiming_it() {
    let d = tempfile::tempdir().unwrap();
    let p = Paths::under(d.path());
    std::fs::create_dir_all(p.theme_dir.join("kde")).unwrap();
    std::fs::write(p.theme_dir.join("kde/uq.colors"), SCHEME).unwrap();
    let path = conf_path(&p);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let theirs = "[General]\nColorScheme=BreezeDark\n\n[Colors:Window]\nBackgroundNormal=9,9,9\n";
    std::fs::write(&path, theirs).unwrap();
    assert!(
        !sync_look(&p, &look("Papirus-Dark"), false).unwrap(),
        "off: kept"
    );
    assert!(sync_look(&p, &look("Papirus-Dark"), true).unwrap());
    assert_eq!(
        std::fs::read_to_string(path.with_file_name("kdeglobals.before-uq")).unwrap(),
        theirs
    );
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("ColorScheme=uQ")
    );
}
