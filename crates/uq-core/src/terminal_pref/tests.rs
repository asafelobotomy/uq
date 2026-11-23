use super::*;
use crate::kdeglobals::{with_chosen_terminal, without_chosen_terminal};

fn setup() -> (tempfile::TempDir, Paths, Vec<PathBuf>) {
    let t = tempfile::tempdir().unwrap();
    let p = Paths::under(t.path());
    let apps = t.path().join("apps");
    std::fs::create_dir_all(&apps).unwrap();
    std::fs::create_dir_all(kdeglobals::conf_path(&p).parent().unwrap()).unwrap();
    for (id, exec) in [
        ("foot.desktop", "foot"),
        ("org.wezfurlong.wezterm.desktop", "wezterm start --cwd ."),
    ] {
        std::fs::write(
            apps.join(id),
            format!("[Desktop Entry]\nType=Application\nName=T\nExec={exec}\n"),
        )
        .unwrap();
    }
    (t, p, vec![apps])
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn list_puts_the_choice_first_once() {
    assert_eq!(
        list_with_first("", "foot.desktop").unwrap(),
        "foot.desktop\n"
    );
    assert_eq!(
        list_with_first(
            "# mine\nkitty.desktop\nfoot.desktop:new-window\n",
            "foot.desktop"
        )
        .unwrap(),
        "foot.desktop\n# mine\nkitty.desktop\n"
    );
    assert_eq!(list_with_first("# c\nfoot.desktop\n", "foot.desktop"), None);
    assert_eq!(
        list_without("foot.desktop\nkitty.desktop\n", "foot.desktop").unwrap(),
        "kitty.desktop\n"
    );
    assert_eq!(list_without("kitty.desktop\n", "foot.desktop"), None);
}

#[test]
fn kde_choice_replaces_any_terminal_and_keeps_the_rest() {
    let old = "[General]\nTerminalApplication=konsole\nfixed=Mono,10\n\n[Icons]\nTheme=Papirus\n";
    let new = with_chosen_terminal(old, "foot", "foot.desktop").unwrap();
    assert_eq!(
        new,
        "[General]\nTerminalService=foot.desktop\nTerminalApplication=foot\nfixed=Mono,10\n\n[Icons]\nTheme=Papirus\n"
    );
    assert_eq!(with_chosen_terminal(&new, "foot", "foot.desktop"), None);
    assert_eq!(
        with_chosen_terminal("", "foot", "foot.desktop").unwrap(),
        "[General]\nTerminalService=foot.desktop\nTerminalApplication=foot\n"
    );
    assert_eq!(
        with_chosen_terminal("[Icons]\nTheme=X\n", "foot", "foot.desktop").unwrap(),
        "[General]\nTerminalService=foot.desktop\nTerminalApplication=foot\n\n[Icons]\nTheme=X\n"
    );
    // Back to the default: only uQ's own choice is taken out.
    assert_eq!(
        without_chosen_terminal(&new, "foot.desktop").unwrap(),
        "[General]\nfixed=Mono,10\n\n[Icons]\nTheme=Papirus\n"
    );
    assert_eq!(
        without_chosen_terminal(
            "[General]\nTerminalService=org.kde.konsole.desktop\n",
            "foot.desktop"
        ),
        None
    );
}

#[test]
fn a_choice_goes_to_kde_and_xdg_terminal_exec() {
    let (_t, p, dirs) = setup();
    std::fs::write(
        kdeglobals::conf_path(&p),
        "[General]\nTerminalApplication=konsole\n",
    )
    .unwrap();
    std::fs::write(list_path(&p), "kitty.desktop\n").unwrap();
    let id = "org.wezfurlong.wezterm.desktop";
    assert!(apply(&p, &dirs, Some(id), None).unwrap());
    assert_eq!(
        read(&kdeglobals::conf_path(&p)),
        format!("[General]\nTerminalService={id}\nTerminalApplication=wezterm start --cwd .\n")
    );
    assert_eq!(read(&list_path(&p)), format!("{id}\nkitty.desktop\n"));
    assert!(
        !apply(&p, &dirs, Some(id), Some(id)).unwrap(),
        "nothing new at login"
    );

    // Back to uQ's default: the old choice goes, what was there before stays.
    assert!(apply(&p, &dirs, None, Some(id)).unwrap());
    assert!(!read(&kdeglobals::conf_path(&p)).contains("wezterm"));
    assert_eq!(read(&list_path(&p)), "kitty.desktop\n");
}

#[test]
fn a_choice_set_elsewhere_survives_going_back_to_the_default() {
    let (_t, p, dirs) = setup();
    let mine = "[General]\nTerminalService=org.kde.konsole.desktop\nTerminalApplication=konsole\n";
    std::fs::write(kdeglobals::conf_path(&p), mine).unwrap();
    std::fs::write(list_path(&p), "org.kde.konsole.desktop\n").unwrap();
    apply(&p, &dirs, None, Some("foot.desktop")).unwrap();
    assert_eq!(read(&kdeglobals::conf_path(&p)), mine);
    assert_eq!(read(&list_path(&p)), "org.kde.konsole.desktop\n");
}

#[test]
fn a_choice_no_longer_installed_changes_nothing_it_named() {
    let (_t, p, dirs) = setup();
    apply(&p, &dirs, Some("gone.desktop"), None).unwrap();
    assert!(!read(&kdeglobals::conf_path(&p)).contains("gone"));
    assert!(!read(&list_path(&p)).contains("gone"));
}
