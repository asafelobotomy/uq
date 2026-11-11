//! The closed set of things a shortcut of your own can do, and the Lua each renders.

use super::hypr::SHELL_NAMES;
use crate::desktop_entry::{normalise_id, valid_desktop_id};
use crate::lua::lua_string;
use crate::settings::{SettingError, invalid};
use serde::{Deserialize, Serialize};

pub const MAX_COMMAND_CHARS: usize = 1024;

const OSD_NAMES: [&str; 6] = [
    "vol-up",
    "vol-down",
    "mute",
    "mic-mute",
    "bright-up",
    "bright-down",
];
const DIRECTIONS: [&str; 4] = ["left", "right", "up", "down"];
const LAYOUTS: [&str; 3] = ["dwindle", "master", "scrolling"];
const MEDIA: [&str; 3] = ["next", "play-pause", "previous"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Shell {
        name: String,
    },
    Window {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        direction: Option<String>,
    },
    Workspace {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workspace: Option<u8>,
    },
    Uq {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        layout: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        media: Option<String>,
    },
    Launch {
        app: String,
    },
    Command {
        command: String,
    },
}

/// POSIX single quoting: `'`, the text with each `'` written `'\''`, `'`.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn choice(key: &str, what: &str, v: Option<&String>, set: &[&str]) -> Result<(), SettingError> {
    match v {
        Some(v) if set.contains(&v.as_str()) => Ok(()),
        Some(_) => Err(invalid(key, format!("{what} isn't one uQ knows"))),
        None => Err(invalid(key, format!("choose {what}"))),
    }
}

fn absent<T>(key: &str, v: &Option<T>) -> Result<(), SettingError> {
    match v {
        None => Ok(()),
        Some(_) => Err(invalid(key, "this isn't used by this action")),
    }
}

impl Action {
    /// Checks the closed names and values. Returns the action with the launch id normalised.
    pub fn check(&self) -> Result<Action, SettingError> {
        self.check_at("action")
    }

    /// As [`Action::check`], naming `key` in a refusal.
    pub fn check_at(&self, key: &str) -> Result<Action, SettingError> {
        let k = |f: &str| format!("{key}.{f}");
        match self {
            Action::Shell { name } => {
                if !SHELL_NAMES.contains(&name.as_str()) {
                    return Err(invalid(&k("name"), "this isn't one of uQ's surfaces"));
                }
            }
            Action::Window { name, direction } => match name.as_str() {
                "close" | "float" | "pin" => absent(&k("direction"), direction)?,
                "focus" => choice(
                    &k("direction"),
                    "a direction",
                    direction.as_ref(),
                    &DIRECTIONS,
                )?,
                _ => return Err(invalid(&k("name"), "this isn't a window action")),
            },
            Action::Workspace { name, workspace } => match name.as_str() {
                "focus" | "move" => match workspace {
                    Some(1..=10) => {}
                    _ => return Err(invalid(&k("workspace"), "choose a workspace from 1 to 10")),
                },
                "toggle_special" | "move_special" | "next" | "prev" => {
                    absent(&k("workspace"), workspace)?
                }
                _ => return Err(invalid(&k("name"), "this isn't a workspace action")),
            },
            Action::Uq {
                name,
                layout,
                media,
            } => match name.as_str() {
                "terminal" | "exit" | "profile-next" | "settings" => {
                    absent(&k("layout"), layout)?;
                    absent(&k("media"), media)?;
                }
                "layout" => {
                    choice(&k("layout"), "a layout", layout.as_ref(), &LAYOUTS)?;
                    absent(&k("media"), media)?;
                }
                "media" => {
                    choice(&k("media"), "a media key", media.as_ref(), &MEDIA)?;
                    absent(&k("layout"), layout)?;
                }
                _ => return Err(invalid(&k("name"), "this isn't one of uQ's actions")),
            },
            Action::Launch { app } => {
                if !valid_desktop_id(app) {
                    return Err(invalid(&k("app"), "this isn't an app id uQ can start"));
                }
                return Ok(Action::Launch {
                    app: normalise_id(app),
                });
            }
            Action::Command { command } => {
                let n = command.chars().count();
                if n == 0 || command.trim().is_empty() {
                    return Err(invalid(&k("command"), "type a command to run"));
                }
                if n > MAX_COMMAND_CHARS {
                    return Err(invalid(&k("command"), "at most 1024 characters"));
                }
                if command.chars().any(char::is_control) {
                    return Err(invalid(
                        &k("command"),
                        "a command is one line of plain text",
                    ));
                }
            }
        }
        Ok(self.clone())
    }

    /// The dispatcher expression, exactly as `generated.lua` holds it. Assumes [`check`] passed.
    pub fn lua(&self) -> String {
        let exec = |c: &str| format!("hl.dsp.exec_cmd({})", lua_string(c));
        match self {
            Action::Shell { name } => {
                format!("hl.dsp.global({})", lua_string(&format!("uq:{name}")))
            }
            Action::Window { name, direction } => match name.as_str() {
                "close" => "hl.dsp.window.close()".into(),
                "float" => "hl.dsp.window.float({ action = \"toggle\" })".into(),
                "pin" => "hl.dsp.window.pin({ action = \"toggle\" })".into(),
                _ => format!(
                    "hl.dsp.focus({{ direction = {} }})",
                    lua_string(direction.as_deref().unwrap_or("left"))
                ),
            },
            Action::Workspace { name, workspace } => {
                let n = workspace.unwrap_or(1);
                match name.as_str() {
                    "focus" => format!("hl.dsp.focus({{ workspace = {n} }})"),
                    "move" => format!("hl.dsp.window.move({{ workspace = {n} }})"),
                    "toggle_special" => "hl.dsp.workspace.toggle_special(\"magic\")".into(),
                    "move_special" => {
                        "hl.dsp.window.move({ workspace = \"special:magic\" })".into()
                    }
                    "next" => "hl.dsp.focus({ workspace = \"e+1\" })".into(),
                    _ => "hl.dsp.focus({ workspace = \"e-1\" })".into(),
                }
            }
            Action::Uq {
                name,
                layout,
                media,
            } => match name.as_str() {
                "terminal" => exec("kitty"),
                "exit" => exec(
                    "command -v hyprshutdown >/dev/null 2>&1 && hyprshutdown || hyprctl dispatch 'hl.dsp.exit()'",
                ),
                // From the closed lists, so a value that skipped the check still can't reach `sh`.
                "layout" => exec(&format!(
                    "uqctl layout set current {}",
                    pick(&LAYOUTS, layout.as_deref(), "dwindle")
                )),
                "media" => exec(&format!(
                    "playerctl {}",
                    pick(&MEDIA, media.as_deref(), "play-pause")
                )),
                "profile-next" => exec("uqctl profile next"),
                _ => exec("uqctl settings"),
            },
            Action::Launch { app } => exec(&format!("uqctl launch {}", shell_quote(app))),
            Action::Command { command } => exec(&format!(
                "systemd-run --user --scope --quiet --collect --slice=app.slice -- sh -c {}",
                shell_quote(command)
            )),
        }
    }

    /// Plain words for the page and history. A command's text is never included.
    pub fn text(&self) -> String {
        match self {
            Action::Shell { name } => match name.as_str() {
                "spotlight" => "Opens Spotlight",
                "appmenu" => "Opens the app menu",
                "quick-settings" => "Opens quick settings",
                "notif-centre" => "Opens notifications",
                "lock" => "Locks the screen",
                "power-menu" => "Opens the power menu",
                "vol-up" => "Turns the volume up",
                "vol-down" => "Turns the volume down",
                "mute" => "Mutes the sound",
                "mic-mute" => "Mutes the microphone",
                "bright-up" => "Turns the brightness up",
                _ => "Turns the brightness down",
            }
            .into(),
            Action::Window { name, direction } => match name.as_str() {
                "close" => "Closes the window".into(),
                "float" => "Floats or tiles the window".into(),
                "pin" => "Pins the window".into(),
                _ => format!(
                    "Focuses the window to the {}",
                    direction.as_deref().unwrap_or("left")
                ),
            },
            Action::Workspace { name, workspace } => {
                let n = workspace.unwrap_or(1);
                match name.as_str() {
                    "focus" => format!("Goes to workspace {n}"),
                    "move" => format!("Moves the window to workspace {n}"),
                    "toggle_special" => "Shows or hides the scratchpad".into(),
                    "move_special" => "Moves the window to the scratchpad".into(),
                    "next" => "Goes to the next workspace".into(),
                    _ => "Goes to the previous workspace".into(),
                }
            }
            Action::Uq {
                name,
                layout,
                media,
            } => match name.as_str() {
                "terminal" => "Opens a terminal".into(),
                "exit" => "Logs out".into(),
                "layout" => format!(
                    "Uses the {} layout here",
                    layout.as_deref().unwrap_or("dwindle")
                ),
                "media" => match media.as_deref() {
                    Some("next") => "Skips to the next track".into(),
                    Some("previous") => "Goes back a track".into(),
                    _ => "Plays or pauses".into(),
                },
                "profile-next" => "Switches to the next profile".into(),
                _ => "Opens Settings".into(),
            },
            Action::Launch { app } => format!("Starts {}", app_name(app)),
            Action::Command { .. } => "Runs a command".into(),
        }
    }

    /// R7: whether the shortcut may work over the lock screen.
    pub fn may_lock(&self) -> bool {
        match self {
            Action::Shell { name } => OSD_NAMES.contains(&name.as_str()),
            Action::Uq { name, .. } => name == "media",
            _ => false,
        }
    }

    /// R7: whether holding the key may repeat it.
    /// Only the volume-style actions make sense held down: starting an app, a command or a
    /// terminal over and over is the harm the refusal names.
    pub fn may_repeat(&self) -> bool {
        match self {
            Action::Uq { name, .. } => name == "media",
            Action::Launch { .. } | Action::Command { .. } => false,
            _ => true,
        }
    }
}

/// The entry of `list` equal to `v`, else `fallback`.
fn pick(list: &'static [&'static str], v: Option<&str>, fallback: &'static str) -> &'static str {
    list.iter()
        .copied()
        .find(|x| Some(*x) == v)
        .unwrap_or(fallback)
}

/// Every action a shortcut of your own can pick from a list, grouped as the page shows them:
/// `shell`, `window`, `workspace` and `uq`. (`launch` and `command` take text, so they aren't
/// listed.) The page never copies this list.
pub fn catalogue() -> Vec<(&'static str, Action)> {
    let mut v = Vec::new();
    for n in SHELL_NAMES {
        v.push(("shell", Action::Shell { name: n.into() }));
    }
    for n in ["close", "float", "pin"] {
        v.push((
            "window",
            Action::Window {
                name: n.into(),
                direction: None,
            },
        ));
    }
    for d in DIRECTIONS {
        v.push((
            "window",
            Action::Window {
                name: "focus".into(),
                direction: Some(d.into()),
            },
        ));
    }
    for n in ["focus", "move"] {
        for w in 1..=10 {
            v.push((
                "workspace",
                Action::Workspace {
                    name: n.into(),
                    workspace: Some(w),
                },
            ));
        }
    }
    for n in ["toggle_special", "move_special", "next", "prev"] {
        v.push((
            "workspace",
            Action::Workspace {
                name: n.into(),
                workspace: None,
            },
        ));
    }
    let uq = |name: &str, layout: Option<&str>, media: Option<&str>| {
        (
            "uq",
            Action::Uq {
                name: name.into(),
                layout: layout.map(Into::into),
                media: media.map(Into::into),
            },
        )
    };
    v.push(uq("terminal", None, None));
    v.push(uq("exit", None, None));
    for l in LAYOUTS {
        v.push(uq("layout", Some(l), None));
    }
    for m in MEDIA {
        v.push(uq("media", None, Some(m)));
    }
    v.push(uq("profile-next", None, None));
    v.push(uq("settings", None, None));
    v
}

/// A readable name from a desktop id: the last part, capitalised.
fn app_name(app: &str) -> String {
    let id = app.strip_suffix(".desktop").unwrap_or(app);
    let last: String = id
        .rsplit('.')
        .next()
        .unwrap_or(id)
        .chars()
        .filter(|c| !c.is_control())
        .take(40)
        .collect();
    let mut cs = last.chars();
    match cs.next() {
        Some(f) => f.to_uppercase().chain(cs).collect(),
        None => "an app".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybinds::hypr::DISPATCHERS;

    fn all() -> Vec<Action> {
        let s = |n: &str| Action::Shell { name: n.into() };
        let mut v: Vec<Action> = SHELL_NAMES.iter().map(|n| s(n)).collect();
        for n in ["close", "float", "pin"] {
            v.push(Action::Window {
                name: n.into(),
                direction: None,
            });
        }
        for d in DIRECTIONS {
            v.push(Action::Window {
                name: "focus".into(),
                direction: Some(d.into()),
            });
        }
        for n in ["focus", "move"] {
            v.push(Action::Workspace {
                name: n.into(),
                workspace: Some(10),
            });
        }
        for n in ["toggle_special", "move_special", "next", "prev"] {
            v.push(Action::Workspace {
                name: n.into(),
                workspace: None,
            });
        }
        for n in ["terminal", "exit", "profile-next", "settings"] {
            v.push(Action::Uq {
                name: n.into(),
                layout: None,
                media: None,
            });
        }
        for l in LAYOUTS {
            v.push(Action::Uq {
                name: "layout".into(),
                layout: Some(l.into()),
                media: None,
            });
        }
        for m in MEDIA {
            v.push(Action::Uq {
                name: "media".into(),
                layout: None,
                media: Some(m.into()),
            });
        }
        v.push(Action::Launch {
            app: "org.mozilla.firefox".into(),
        });
        v.push(Action::Command {
            command: "echo hi".into(),
        });
        v
    }

    #[test]
    fn every_action_renders_a_known_dispatcher() {
        for a in all() {
            let a = a.check().unwrap_or_else(|e| panic!("{a:?}: {e}"));
            let lua = a.lua();
            let rest = lua
                .strip_prefix("hl.dsp.")
                .unwrap_or_else(|| panic!("{lua}"));
            let name = rest.split('(').next().unwrap();
            assert!(DISPATCHERS.contains(&name), "{lua}");
            assert!(!lua.contains('\n'));
            assert!(!a.text().is_empty());
        }
    }

    #[test]
    fn shell_quote_round_trips() {
        if std::process::Command::new("sh")
            .arg("-c")
            .arg("true")
            .status()
            .is_err()
        {
            eprintln!("skipped: no sh");
            return;
        }
        let long = "x'y\"z\\$ ".repeat(128);
        for s in ["a'b", "''", "$(x)", "\"", "\\", long.as_str(), ""] {
            // The shell parses `printf %s <quoted>`; the one word it gets must be `s` itself.
            let via = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("printf %s {}", shell_quote(s)))
                .output()
                .unwrap();
            assert_eq!(String::from_utf8_lossy(&via.stdout), s);
        }
    }

    #[test]
    fn launch_renders_uqctl_launch() {
        let a = Action::Launch {
            app: "org.mozilla.firefox".into(),
        }
        .check()
        .unwrap();
        assert_eq!(
            a.lua(),
            "hl.dsp.exec_cmd(\"uqctl launch 'org.mozilla.firefox.desktop'\")"
        );
        assert_eq!(a.text(), "Starts Firefox");
    }

    #[test]
    fn command_renders_a_scope() {
        let a = Action::Command {
            command: "grim -g \"$(slurp)\"".into(),
        };
        assert_eq!(
            a.lua(),
            "hl.dsp.exec_cmd(\"systemd-run --user --scope --quiet --collect --slice=app.slice -- sh -c 'grim -g \\\"$(slurp)\\\"'\")"
        );
        assert_eq!(a.text(), "Runs a command");
    }

    #[test]
    fn closed_values() {
        let json = |s: &str| serde_json::from_str::<Action>(s);
        for bad in [
            r#"{"kind":"shell","name":"nope"}"#,
            r#"{"kind":"workspace","name":"focus","workspace":11}"#,
            r#"{"kind":"workspace","name":"focus"}"#,
            r#"{"kind":"window","name":"focus","direction":"diag"}"#,
            r#"{"kind":"uq","name":"layout","layout":"monocle"}"#,
            r#"{"kind":"launch","app":"x.desktop; rm -rf ~"}"#,
            r#"{"kind":"command","command":""}"#,
            r#"{"kind":"command","command":"a\nb"}"#,
        ] {
            assert!(json(bad).unwrap().check().is_err(), "{bad}");
        }
        assert!(
            json(r#"{"kind":"command","command":"x"}"#)
                .unwrap()
                .check()
                .is_ok()
        );
        assert!(json(r#"{"kind":"fly","name":"x"}"#).is_err());
        assert!(json(r#"{"kind":"shell","name":"lock","extra":1}"#).is_err());
        let long = Action::Command {
            command: "a".repeat(1025),
        };
        assert!(long.check().is_err());
        assert!(
            Action::Command {
                command: "a".repeat(1024)
            }
            .check()
            .is_ok()
        );
    }

    #[test]
    fn lock_and_repeat_limits() {
        for a in all() {
            let osd = matches!(&a, Action::Shell { name } if OSD_NAMES.contains(&name.as_str()));
            let media = matches!(&a, Action::Uq { name, .. } if name == "media");
            assert_eq!(a.may_lock(), osd || media, "{a:?}");
            // Only things that start something are refused; the uQ exec actions start one.
            let starts = matches!(a, Action::Launch { .. } | Action::Command { .. })
                || matches!(&a, Action::Uq { name, .. } if name != "media");
            assert_eq!(a.may_repeat(), !starts, "{a:?}");
        }
    }
}
