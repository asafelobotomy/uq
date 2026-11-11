//! uQ's default shortcuts: the table `base.lua` used to hold, in `base.lua` order (also the
//! render order). Each row keeps `base.lua`'s exact Lua (R4); a default changes no behaviour.

use crate::lua::lua_string;

pub const LOCK_RECOVERY: &str = "uq.lock-recovery";
pub const PROFILE_NEXT: &str = "uq.profile.next";

/// The page's sections, in page order. (Not [`super::keys::Group`], which sorts keys.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    AppsAndDesktop,
    Windows,
    Workspaces,
    SoundAndBrightness,
    Media,
    Profiles,
}

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Group::AppsAndDesktop => "Apps and desktop",
            Group::Windows => "Windows",
            Group::Workspaces => "Workspaces",
            Group::SoundAndBrightness => "Sound and brightness",
            Group::Media => "Media",
            Group::Profiles => "Profiles",
        }
    }
}

/// A default's dispatcher, as `'static` text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultAction {
    /// `hl.dsp.exec_cmd("<text>")`.
    Exec(&'static str),
    /// `hl.dsp.global("uq:<name>")`.
    Global(&'static str),
    /// The dispatcher call, verbatim.
    Raw(&'static str),
}

impl DefaultAction {
    pub fn lua(&self) -> String {
        match self {
            DefaultAction::Exec(c) => format!("hl.dsp.exec_cmd({})", lua_string(c)),
            DefaultAction::Global(n) => {
                format!("hl.dsp.global({})", lua_string(&format!("uq:{n}")))
            }
            DefaultAction::Raw(s) => (*s).to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Default {
    pub id: &'static str,
    pub label: &'static str,
    pub group: Group,
    /// `None`: unbound by default.
    pub combo: Option<&'static str>,
    pub action: DefaultAction,
    pub locked: bool,
    pub repeating: bool,
    pub release: bool,
    pub mouse: bool,
}

pub fn defaults() -> &'static [Default] {
    &DEFAULTS
}

pub fn default_of(id: &str) -> Option<&'static Default> {
    DEFAULTS.iter().find(|d| d.id == id)
}

static DEFAULTS: [Default; 55] = [
    Default {
        id: "uq.terminal",
        label: "Open a terminal",
        group: Group::AppsAndDesktop,
        combo: Some("SUPER + Return"),
        action: DefaultAction::Exec("kitty"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.close",
        label: "Close the window",
        group: Group::Windows,
        combo: Some("SUPER + Q"),
        action: DefaultAction::Raw("hl.dsp.window.close()"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.float",
        label: "Float or tile the window",
        group: Group::Windows,
        combo: Some("SUPER + V"),
        action: DefaultAction::Raw("hl.dsp.window.float({ action = \"toggle\" })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.exit",
        label: "Log out",
        group: Group::AppsAndDesktop,
        combo: Some("SUPER + SHIFT + M"),
        action: DefaultAction::Exec(
            "command -v hyprshutdown >/dev/null 2>&1 && hyprshutdown || hyprctl dispatch 'hl.dsp.exit()'",
        ),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.spotlight",
        label: "Open Spotlight",
        group: Group::AppsAndDesktop,
        combo: Some("SUPER + Space"),
        action: DefaultAction::Global("spotlight"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.appmenu",
        label: "Open the app menu",
        group: Group::AppsAndDesktop,
        combo: Some("SUPER + SUPER_L"),
        action: DefaultAction::Global("appmenu"),
        locked: false,
        repeating: false,
        release: true,
        mouse: false,
    },
    Default {
        id: "uq.quick-settings",
        label: "Open quick settings",
        group: Group::AppsAndDesktop,
        combo: Some("SUPER + A"),
        action: DefaultAction::Global("quick-settings"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.notif-centre",
        label: "Open notifications",
        group: Group::AppsAndDesktop,
        combo: Some("SUPER + N"),
        action: DefaultAction::Global("notif-centre"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.lock",
        label: "Lock the screen",
        group: Group::AppsAndDesktop,
        combo: Some("SUPER + L"),
        action: DefaultAction::Global("lock"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.power-menu",
        label: "Open the power menu",
        group: Group::AppsAndDesktop,
        combo: Some("SUPER + Escape"),
        action: DefaultAction::Global("power-menu"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.layout.dwindle",
        label: "Use the dwindle layout here",
        group: Group::Windows,
        combo: Some("SUPER + ALT + D"),
        action: DefaultAction::Exec("uqctl layout set current dwindle"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.layout.master",
        label: "Use the master layout here",
        group: Group::Windows,
        combo: Some("SUPER + ALT + M"),
        action: DefaultAction::Exec("uqctl layout set current master"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.layout.scrolling",
        label: "Use the scrolling layout here",
        group: Group::Windows,
        combo: Some("SUPER + ALT + S"),
        action: DefaultAction::Exec("uqctl layout set current scrolling"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.focus.left",
        label: "Focus the window to the left",
        group: Group::Windows,
        combo: Some("SUPER + left"),
        action: DefaultAction::Raw("hl.dsp.focus({ direction = \"left\" })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.focus.right",
        label: "Focus the window to the right",
        group: Group::Windows,
        combo: Some("SUPER + right"),
        action: DefaultAction::Raw("hl.dsp.focus({ direction = \"right\" })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.focus.up",
        label: "Focus the window above",
        group: Group::Windows,
        combo: Some("SUPER + up"),
        action: DefaultAction::Raw("hl.dsp.focus({ direction = \"up\" })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.focus.down",
        label: "Focus the window below",
        group: Group::Windows,
        combo: Some("SUPER + down"),
        action: DefaultAction::Raw("hl.dsp.focus({ direction = \"down\" })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.focus.1",
        label: "Go to workspace 1",
        group: Group::Workspaces,
        combo: Some("SUPER + 1"),
        action: DefaultAction::Raw("hl.dsp.focus({ workspace = 1 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.move.1",
        label: "Move the window to workspace 1",
        group: Group::Workspaces,
        combo: Some("SUPER + SHIFT + 1"),
        action: DefaultAction::Raw("hl.dsp.window.move({ workspace = 1 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.focus.2",
        label: "Go to workspace 2",
        group: Group::Workspaces,
        combo: Some("SUPER + 2"),
        action: DefaultAction::Raw("hl.dsp.focus({ workspace = 2 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.move.2",
        label: "Move the window to workspace 2",
        group: Group::Workspaces,
        combo: Some("SUPER + SHIFT + 2"),
        action: DefaultAction::Raw("hl.dsp.window.move({ workspace = 2 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.focus.3",
        label: "Go to workspace 3",
        group: Group::Workspaces,
        combo: Some("SUPER + 3"),
        action: DefaultAction::Raw("hl.dsp.focus({ workspace = 3 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.move.3",
        label: "Move the window to workspace 3",
        group: Group::Workspaces,
        combo: Some("SUPER + SHIFT + 3"),
        action: DefaultAction::Raw("hl.dsp.window.move({ workspace = 3 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.focus.4",
        label: "Go to workspace 4",
        group: Group::Workspaces,
        combo: Some("SUPER + 4"),
        action: DefaultAction::Raw("hl.dsp.focus({ workspace = 4 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.move.4",
        label: "Move the window to workspace 4",
        group: Group::Workspaces,
        combo: Some("SUPER + SHIFT + 4"),
        action: DefaultAction::Raw("hl.dsp.window.move({ workspace = 4 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.focus.5",
        label: "Go to workspace 5",
        group: Group::Workspaces,
        combo: Some("SUPER + 5"),
        action: DefaultAction::Raw("hl.dsp.focus({ workspace = 5 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.move.5",
        label: "Move the window to workspace 5",
        group: Group::Workspaces,
        combo: Some("SUPER + SHIFT + 5"),
        action: DefaultAction::Raw("hl.dsp.window.move({ workspace = 5 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.focus.6",
        label: "Go to workspace 6",
        group: Group::Workspaces,
        combo: Some("SUPER + 6"),
        action: DefaultAction::Raw("hl.dsp.focus({ workspace = 6 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.move.6",
        label: "Move the window to workspace 6",
        group: Group::Workspaces,
        combo: Some("SUPER + SHIFT + 6"),
        action: DefaultAction::Raw("hl.dsp.window.move({ workspace = 6 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.focus.7",
        label: "Go to workspace 7",
        group: Group::Workspaces,
        combo: Some("SUPER + 7"),
        action: DefaultAction::Raw("hl.dsp.focus({ workspace = 7 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.move.7",
        label: "Move the window to workspace 7",
        group: Group::Workspaces,
        combo: Some("SUPER + SHIFT + 7"),
        action: DefaultAction::Raw("hl.dsp.window.move({ workspace = 7 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.focus.8",
        label: "Go to workspace 8",
        group: Group::Workspaces,
        combo: Some("SUPER + 8"),
        action: DefaultAction::Raw("hl.dsp.focus({ workspace = 8 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.move.8",
        label: "Move the window to workspace 8",
        group: Group::Workspaces,
        combo: Some("SUPER + SHIFT + 8"),
        action: DefaultAction::Raw("hl.dsp.window.move({ workspace = 8 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.focus.9",
        label: "Go to workspace 9",
        group: Group::Workspaces,
        combo: Some("SUPER + 9"),
        action: DefaultAction::Raw("hl.dsp.focus({ workspace = 9 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.move.9",
        label: "Move the window to workspace 9",
        group: Group::Workspaces,
        combo: Some("SUPER + SHIFT + 9"),
        action: DefaultAction::Raw("hl.dsp.window.move({ workspace = 9 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.focus.10",
        label: "Go to workspace 10",
        group: Group::Workspaces,
        combo: Some("SUPER + 0"),
        action: DefaultAction::Raw("hl.dsp.focus({ workspace = 10 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.move.10",
        label: "Move the window to workspace 10",
        group: Group::Workspaces,
        combo: Some("SUPER + SHIFT + 0"),
        action: DefaultAction::Raw("hl.dsp.window.move({ workspace = 10 })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.special.toggle",
        label: "Show or hide the scratchpad",
        group: Group::Workspaces,
        combo: Some("SUPER + S"),
        action: DefaultAction::Raw("hl.dsp.workspace.toggle_special(\"magic\")"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.special.move",
        label: "Move the window to the scratchpad",
        group: Group::Workspaces,
        combo: Some("SUPER + SHIFT + S"),
        action: DefaultAction::Raw("hl.dsp.window.move({ workspace = \"special:magic\" })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.next",
        label: "Next workspace",
        group: Group::Workspaces,
        combo: Some("SUPER + mouse_down"),
        action: DefaultAction::Raw("hl.dsp.focus({ workspace = \"e+1\" })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.ws.prev",
        label: "Previous workspace",
        group: Group::Workspaces,
        combo: Some("SUPER + mouse_up"),
        action: DefaultAction::Raw("hl.dsp.focus({ workspace = \"e-1\" })"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.window.drag",
        label: "Move the window with the mouse",
        group: Group::Windows,
        combo: Some("SUPER + mouse:272"),
        action: DefaultAction::Raw("hl.dsp.window.drag()"),
        locked: false,
        repeating: false,
        release: false,
        mouse: true,
    },
    Default {
        id: "uq.window.resize",
        label: "Resize the window with the mouse",
        group: Group::Windows,
        combo: Some("SUPER + mouse:273"),
        action: DefaultAction::Raw("hl.dsp.window.resize()"),
        locked: false,
        repeating: false,
        release: false,
        mouse: true,
    },
    Default {
        id: "uq.lock-recovery",
        label: "Restart the lock screen if it stops working",
        group: Group::AppsAndDesktop,
        combo: Some("SUPER + CTRL + L"),
        action: DefaultAction::Exec("systemctl --user restart uq-lock.service || hyprlock"),
        locked: true,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.vol-up",
        label: "Volume up",
        group: Group::SoundAndBrightness,
        combo: Some("XF86AudioRaiseVolume"),
        action: DefaultAction::Global("vol-up"),
        locked: true,
        repeating: true,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.vol-down",
        label: "Volume down",
        group: Group::SoundAndBrightness,
        combo: Some("XF86AudioLowerVolume"),
        action: DefaultAction::Global("vol-down"),
        locked: true,
        repeating: true,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.mute",
        label: "Mute",
        group: Group::SoundAndBrightness,
        combo: Some("XF86AudioMute"),
        action: DefaultAction::Global("mute"),
        locked: true,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.mic-mute",
        label: "Mute the microphone",
        group: Group::SoundAndBrightness,
        combo: Some("XF86AudioMicMute"),
        action: DefaultAction::Global("mic-mute"),
        locked: true,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.bright-up",
        label: "Brightness up",
        group: Group::SoundAndBrightness,
        combo: Some("XF86MonBrightnessUp"),
        action: DefaultAction::Global("bright-up"),
        locked: true,
        repeating: true,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.bright-down",
        label: "Brightness down",
        group: Group::SoundAndBrightness,
        combo: Some("XF86MonBrightnessDown"),
        action: DefaultAction::Global("bright-down"),
        locked: true,
        repeating: true,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.media.next",
        label: "Next track",
        group: Group::Media,
        combo: Some("XF86AudioNext"),
        action: DefaultAction::Exec("playerctl next"),
        locked: true,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.media.pause",
        label: "Pause",
        group: Group::Media,
        combo: Some("XF86AudioPause"),
        action: DefaultAction::Exec("playerctl play-pause"),
        locked: true,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.media.play",
        label: "Play",
        group: Group::Media,
        combo: Some("XF86AudioPlay"),
        action: DefaultAction::Exec("playerctl play-pause"),
        locked: true,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.media.prev",
        label: "Previous track",
        group: Group::Media,
        combo: Some("XF86AudioPrev"),
        action: DefaultAction::Exec("playerctl previous"),
        locked: true,
        repeating: false,
        release: false,
        mouse: false,
    },
    Default {
        id: "uq.profile.next",
        label: "Switch to the next profile",
        group: Group::Profiles,
        combo: None,
        action: DefaultAction::Exec("uqctl profile next"),
        locked: false,
        repeating: false,
        release: false,
        mouse: false,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybinds::combo::parse;
    use std::collections::BTreeSet;

    fn base_lua() -> String {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/hypr/base.lua");
        std::fs::read_to_string(p).expect("base.lua")
    }

    #[test]
    fn table_has_55_rows_54_bound() {
        assert_eq!(defaults().len(), 55);
        assert_eq!(defaults().iter().filter(|d| d.combo.is_some()).count(), 54);
    }

    #[test]
    fn ids_unique_and_well_formed() {
        let mut seen = BTreeSet::new();
        for d in defaults() {
            assert!(seen.insert(d.id), "{}", d.id);
            let rest = d.id.strip_prefix("uq.").expect(d.id);
            assert!((1..=48).contains(&rest.len()), "{}", d.id);
            assert!(
                rest.bytes().all(|b| b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || matches!(b, b'.' | b'-')),
                "{}",
                d.id
            );
            assert!(!d.label.is_empty());
        }
        assert_eq!(default_of("uq.spotlight").unwrap().label, "Open Spotlight");
        assert!(default_of("uq.nope").is_none());
    }

    #[test]
    fn profile_next_is_unbound_and_runs_uqctl() {
        let d = default_of(PROFILE_NEXT).unwrap();
        assert_eq!(d.combo, None);
        assert_eq!(d.action.lua(), "hl.dsp.exec_cmd(\"uqctl profile next\")");
        assert_eq!(
            default_of(LOCK_RECOVERY).unwrap().combo,
            Some("SUPER + CTRL + L")
        );
    }

    #[test]
    fn default_combos_parse_and_never_conflict() {
        let mut seen = BTreeSet::new();
        for d in defaults().iter().filter(|d| d.combo.is_some()) {
            let c = parse(d.combo.unwrap()).unwrap_or_else(|e| panic!("{}: {e}", d.id));
            assert!(seen.insert(c), "{} repeats a combo", d.id);
        }
        assert_eq!(seen.len(), 54);
    }

    #[test]
    fn lua_forms_match_base_lua_text() {
        let base = base_lua();
        assert!(base.contains("local terminal = \"kitty\""));
        for d in defaults().iter().filter(|d| d.combo.is_some()) {
            let lua = d.action.lua();
            let (loop_row, text) = match d.id.split_once("uq.ws.") {
                Some((_, rest)) if rest.starts_with("focus.") || rest.starts_with("move.") => {
                    let n = rest.rsplit('.').next().unwrap();
                    (
                        true,
                        lua.replace(&format!("workspace = {n} "), "workspace = i "),
                    )
                }
                _ => (false, lua.clone()),
            };
            let text = if d.id == "uq.terminal" {
                "hl.dsp.exec_cmd(terminal)".to_string()
            } else {
                text
            };
            assert!(base.contains(&text), "{}: {text} (loop: {loop_row})", d.id);
        }
    }

    #[test]
    fn locked_and_repeating_as_base_lua() {
        let locked: Vec<&str> = defaults()
            .iter()
            .filter(|d| d.locked)
            .map(|d| d.id)
            .collect();
        assert_eq!(
            locked,
            [
                "uq.lock-recovery",
                "uq.vol-up",
                "uq.vol-down",
                "uq.mute",
                "uq.mic-mute",
                "uq.bright-up",
                "uq.bright-down",
                "uq.media.next",
                "uq.media.pause",
                "uq.media.play",
                "uq.media.prev"
            ]
        );
        let rep: Vec<&str> = defaults()
            .iter()
            .filter(|d| d.repeating)
            .map(|d| d.id)
            .collect();
        assert_eq!(
            rep,
            ["uq.vol-up", "uq.vol-down", "uq.bright-up", "uq.bright-down"]
        );
        // The launcher rows are never locked; base.lua says the same line by line.
        for id in [
            "uq.spotlight",
            "uq.appmenu",
            "uq.quick-settings",
            "uq.notif-centre",
            "uq.lock",
            "uq.power-menu",
        ] {
            assert!(!default_of(id).unwrap().locked, "{id}");
        }
        let base = base_lua();
        for d in defaults()
            .iter()
            .filter(|d| d.combo.is_some() && d.id != "uq.terminal")
        {
            let lua = d.action.lua();
            if d.id.starts_with("uq.ws.focus") || d.id.starts_with("uq.ws.move") {
                continue;
            }
            let line = base.lines().find(|l| l.contains(&lua)).expect(d.id);
            assert_eq!(line.contains("locked = true"), d.locked, "{}", d.id);
            assert_eq!(line.contains("repeating = true"), d.repeating, "{}", d.id);
            assert_eq!(line.contains("release = true"), d.release, "{}", d.id);
            assert_eq!(line.contains("mouse = true"), d.mouse, "{}", d.id);
        }
    }
}
