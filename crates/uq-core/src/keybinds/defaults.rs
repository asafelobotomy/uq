//! uQ's default shortcuts: the table `base.lua` used to hold, in `base.lua` order (also the
//! render order). Each row keeps `base.lua`'s exact Lua (R4); a default changes no behaviour.

use crate::lua::lua_string;
mod table;
use table::*;

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
    /// The page's name for the section, in `snake_case`.
    pub fn id(self) -> &'static str {
        match self {
            Group::AppsAndDesktop => "apps_and_desktop",
            Group::Windows => "windows",
            Group::Workspaces => "workspaces",
            Group::SoundAndBrightness => "sound_and_brightness",
            Group::Media => "media",
            Group::Profiles => "profiles",
        }
    }

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

/// The terminal shortcut's command: the terminal chosen in Settings › Default apps, started by
/// `uqctl launch --terminal` the way the launcher starts apps (its own scope in `app.slice`, its
/// fix from Apps and scaling). Should that fail (no uqctl, no Hyprland socket), kyko, or kitty
/// when kyko is not installed, in a scope of its own in `app.slice`, so a scheduler sees it as an
/// app rather than part of Hyprland; `$$` is the shell Hyprland runs it in, unique while the
/// scope lives. config/hypr/base.lua repeats it.
pub const TERMINAL: &str = "uqctl launch --terminal 2>/dev/null || systemd-run --user --scope --quiet --collect --slice=app.slice --unit=app-uq-terminal-$$ -- sh -c 'command -v kyko >/dev/null && exec kyko; exec kitty'";

/// The drop-down terminal's key: `kyko toggle` asks the drop-down over D-Bus, and D-Bus starts
/// it (kyko.service, in its own unit) the first time. kyko sees this bind and doesn't add its
/// own (kyko-hotkey), so a press toggles once.
pub const DROPDOWN: &str = "kyko toggle";

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

#[cfg(test)]
mod tests;
