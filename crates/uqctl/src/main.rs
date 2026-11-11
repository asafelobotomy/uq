//! uqctl: command-line client for uqd (`org.uq.Desktop1`).

mod bridge;
mod launch_cmd;
mod profile_cmd;
mod settings_app;
mod settings_cmd;
mod setup;

use clap::{Parser, Subcommand};
use profile_cmd::ProfileCmd;
use serde_json::Value;
use settings_cmd::{
    IdleCmd, LaunchesCmd, MouseCmd, NotifCmd, OnOff, PinCmd, PrivacyKey, RestoreCmd, WeatherCmd,
};
use std::collections::HashMap;
use std::process::ExitCode;
use uq_core::schedule::Schedule;

const NAME: &str = "org.uq.Desktop1";
const PATH: &str = "/org/uq/Desktop1";
/// The origin uqd's history records for uqctl's writes.
const ORIGIN: &str = "uqctl";

#[derive(Parser)]
#[command(name = "uqctl", about = "Control the uQ desktop", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show, list or change the theme.
    Theme {
        #[command(subcommand)]
        cmd: ThemeCmd,
    },
    /// Show or change per-workspace layouts.
    Layout {
        #[command(subcommand)]
        cmd: LayoutCmd,
    },
    /// Switch dark/light automatically.
    Schedule {
        #[command(subcommand)]
        cmd: ScheduleCmd,
    },
    /// Pin, unpin or list dock apps.
    Pin {
        #[command(subcommand)]
        cmd: PinCmd,
    },
    /// Choose the weather location (or turn weather off).
    Weather {
        #[command(subcommand)]
        cmd: WeatherCmd,
    },
    /// Idle timings: dim, lock, screen-off, suspend, locked-screen-off.
    Idle {
        #[command(subcommand)]
        cmd: IdleCmd,
    },
    /// Notification settings.
    Notif {
        #[command(subcommand)]
        cmd: NotifCmd,
    },
    /// Turn a privacy-relevant feature on or off.
    Privacy { key: PrivacyKey, state: OnOff },
    /// Mouse speed, acceleration, scroll direction and handedness.
    Mouse {
        #[command(subcommand)]
        cmd: MouseCmd,
    },
    /// App launch counts.
    Launches {
        #[command(subcommand)]
        cmd: LaunchesCmd,
    },
    /// Settings profiles: list, turn on and off, switch, export, import or delete.
    Profile {
        #[command(subcommand)]
        cmd: ProfileCmd,
    },
    /// Open the Settings app, optionally on a page and a setting (e.g. `uqctl settings wallpaper`).
    Settings {
        page: Option<String>,
        setting: Option<String>,
    },
    /// Print recent setting changes: id, time, origin and summary, newest first.
    History {
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// Undo a change from `uqctl history` (the newest one if no id is given).
    Undo {
        id: Option<String>,
        /// Undo even if the same settings were changed again since.
        #[arg(long)]
        force: bool,
    },
    /// Restore points: `list`, `save <name>`, `delete <id>`, or `<id>` to restore one.
    Restore {
        #[command(subcommand)]
        cmd: RestoreCmd,
    },
    /// Re-render every config file from uqd's state.
    Reload,
    /// Start an app by its desktop id (e.g. `org.mozilla.firefox`), as the shell does: with its
    /// fix from Apps and scaling, in its own scope.
    Launch { id: String },
    /// Run the shell's JSON-lines bridge on stdin/stdout (started by the shell, not by hand).
    Bridge,
    /// Install uQ's ~/.config/hypr/hyprland.lua (never overwrites an existing one). Safe to run
    /// unattended and repeatedly. Exit 0: uQ's config is in place; 1: error (e.g. uq not
    /// installed); 2: refused as root; 3: your own hyprland.lua was kept.
    Setup {
        /// Also point GTK, qt6ct, kitty, foot and alacritty at uQ's theme. These files apply in
        /// every desktop you log in to, not only Hyprland.
        #[arg(long)]
        apps: bool,
        /// Print the outcome as one JSON object on stdout (for installers such as bindle).
        #[arg(long)]
        json: bool,
        /// Print nothing on success; errors still go to stderr (for `uq-session` at login).
        #[arg(long, conflicts_with = "json")]
        quiet: bool,
    },
}

#[derive(Subcommand)]
enum ThemeCmd {
    Get,
    List,
    /// e.g. `uqctl theme set rosewood light`
    Set {
        id: String,
        variant: String,
    },
}

#[derive(Subcommand)]
enum LayoutCmd {
    Get,
    /// e.g. `uqctl layout set 2 scrolling`; WORKSPACE may be `current`.
    Set {
        workspace: String,
        layout: String,
    },
}

#[derive(Subcommand)]
enum ScheduleCmd {
    Off,
    /// Light from LIGHT_AT to DARK_AT, local time as HH:MM.
    Fixed {
        light_at: String,
        dark_at: String,
    },
    /// Light from sunrise to sunset at LAT LON.
    Sun {
        #[arg(allow_negative_numbers = true)]
        lat: f64,
        #[arg(allow_negative_numbers = true)]
        lon: f64,
    },
}

fn parse_time(s: &str) -> Result<chrono::NaiveTime, String> {
    chrono::NaiveTime::parse_from_str(s, "%H:%M")
        .map_err(|e| format!("{s:?} is not a HH:MM time ({e})"))
}

/// The JSON form `SetSchedule` takes.
fn schedule_json(cmd: &ScheduleCmd) -> Result<String, String> {
    let s = match cmd {
        ScheduleCmd::Off => Schedule::Off,
        ScheduleCmd::Fixed { light_at, dark_at } => Schedule::Fixed {
            light_at: parse_time(light_at)?,
            dark_at: parse_time(dark_at)?,
        },
        ScheduleCmd::Sun { lat, lon } => Schedule::Sun {
            lat: *lat,
            lon: *lon,
        },
    };
    serde_json::to_string(&s).map_err(|e| e.to_string())
}

/// A Hyprland workspace selector for `hyprctl -j activeworkspace` output: the numeric id for
/// normal workspaces, otherwise `special:…` or `name:<name>` (a bare name never matches a rule).
fn active_workspace_selector(json: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("unexpected hyprctl output: {e}"))?;
    if let Some(id) = v["id"].as_i64().filter(|id| *id > 0) {
        return Ok(id.to_string());
    }
    let name = v["name"]
        .as_str()
        .ok_or_else(|| "hyprctl reported no active workspace".to_string())?;
    Ok(if name.starts_with("special") {
        name.to_string()
    } else {
        format!("name:{name}")
    })
}

fn current_workspace() -> Result<String, String> {
    let out = std::process::Command::new("hyprctl")
        .args(["-j", "activeworkspace"])
        .output()
        .map_err(|e| format!("running hyprctl: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "hyprctl failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    active_workspace_selector(&String::from_utf8_lossy(&out.stdout))
}

fn describe(err: zbus::Error) -> String {
    match err {
        zbus::Error::MethodError(_, Some(msg), _) => msg,
        zbus::Error::MethodError(name, None, _) => name.to_string(),
        other => format!("{other} (is uqd running?)"),
    }
}

async fn run(cmd: Cmd) -> Result<(), String> {
    if matches!(cmd, Cmd::Bridge | Cmd::Settings { .. }) {
        unreachable!("handled in main");
    }
    if let Cmd::Profile { cmd } = cmd {
        return profile_cmd::run(&profile_cmd::connect().await?, cmd).await;
    }
    let conn = zbus::Connection::session().await.map_err(describe)?;
    let proxy = zbus::Proxy::new(&conn, NAME, PATH, NAME)
        .await
        .map_err(describe)?;
    match cmd {
        Cmd::Theme { cmd: ThemeCmd::Get } => {
            let (id, variant): (String, String) =
                proxy.call("GetTheme", &()).await.map_err(describe)?;
            println!("{id} {variant}");
        }
        Cmd::Theme {
            cmd: ThemeCmd::List,
        } => {
            let ids: Vec<String> = proxy.call("ListThemes", &()).await.map_err(describe)?;
            ids.iter().for_each(|id| println!("{id}"));
        }
        Cmd::Theme {
            cmd: ThemeCmd::Set { id, variant },
        } => {
            proxy
                .call::<_, _, ()>("SetTheme", &(id, variant, ORIGIN))
                .await
                .map_err(describe)?;
        }
        Cmd::Layout {
            cmd: LayoutCmd::Get,
        } => {
            let layouts: HashMap<String, String> =
                proxy.call("GetLayouts", &()).await.map_err(describe)?;
            let mut rows: Vec<_> = layouts.into_iter().collect();
            rows.sort();
            rows.iter().for_each(|(ws, l)| println!("{ws}\t{l}"));
        }
        Cmd::Layout {
            cmd: LayoutCmd::Set { workspace, layout },
        } => {
            let workspace = if workspace == "current" {
                current_workspace()?
            } else {
                workspace
            };
            proxy
                .call::<_, _, ()>("SetLayout", &(workspace, layout, ORIGIN))
                .await
                .map_err(describe)?;
        }
        Cmd::Schedule { cmd } => {
            proxy
                .call::<_, _, ()>("SetSchedule", &(schedule_json(&cmd)?, ORIGIN))
                .await
                .map_err(describe)?;
        }
        Cmd::History { limit } => {
            let json: String = proxy
                .call("ListHistory", &(limit,))
                .await
                .map_err(describe)?;
            settings_cmd::history_lines(&json)?
                .iter()
                .for_each(|l| println!("{l}"));
        }
        Cmd::Undo { id, force } => {
            let id = match id {
                Some(id) => id,
                None => {
                    let json: String = proxy
                        .call("ListHistory", &(1u32,))
                        .await
                        .map_err(describe)?;
                    settings_cmd::newest_entry_id(&json)?
                        .ok_or_else(|| "nothing to undo".to_string())?
                }
            };
            let new_id = proxy
                .call::<_, _, String>("Undo", &(id, force))
                .await
                .map_err(|e| {
                    let conflict = matches!(&e, zbus::Error::MethodError(n, _, _)
                        if n.as_str() == "org.uq.Desktop1.Error.Conflict");
                    let msg = describe(e);
                    if conflict {
                        format!("{msg} (run again with --force)")
                    } else {
                        msg
                    }
                })?;
            println!("{}", settings_cmd::undo_line(&new_id));
        }
        Cmd::Restore { cmd } => restore(&proxy, cmd).await?,
        Cmd::Reload => proxy
            .call::<_, _, ()>("Reload", &())
            .await
            .map_err(describe)?,
        Cmd::Pin { cmd } => pin(&proxy, cmd).await?,
        Cmd::Weather { cmd } => match cmd {
            WeatherCmd::Off => set_setting(&proxy, ("weather_location", Value::Null)).await?,
            WeatherCmd::Set { city, pick } => {
                // No network means no geocode: fail before anything is written.
                let hits = bridge::net::NetClient::new()
                    .geocode(&city)
                    .await
                    .map_err(|e| format!("looking up {city:?}: {e}"))?;
                let (lines, setting) = settings_cmd::weather_choice(&hits, pick)?;
                lines.iter().for_each(|l| println!("{l}"));
                set_setting(&proxy, setting).await?;
            }
        },
        Cmd::Idle {
            cmd: IdleCmd::Set { stage, value },
        } => set_setting(&proxy, settings_cmd::idle_setting(stage, &value)?).await?,
        Cmd::Notif {
            cmd: NotifCmd::Backend { name },
        } => set_setting(&proxy, settings_cmd::notif_backend_setting(&name)).await?,
        Cmd::Privacy { key, state } => {
            set_setting(&proxy, settings_cmd::privacy_setting(key, state)).await?
        }
        Cmd::Mouse { cmd } => match settings_cmd::mouse_setting(&cmd) {
            Some(setting) => set_setting(&proxy, setting).await?,
            None => {
                let json: String = proxy.call("GetSettings", &()).await.map_err(describe)?;
                let v: Value = serde_json::from_str(&json)
                    .map_err(|e| format!("bad settings from uqd: {e}"))?;
                settings_cmd::mouse_lines(&v)
                    .iter()
                    .for_each(|l| println!("{l}"));
            }
        },
        Cmd::Launches {
            cmd: LaunchesCmd::Clear,
        } => proxy
            .call::<_, _, ()>("ClearLaunchCounts", &())
            .await
            .map_err(describe)?,
        Cmd::Setup { .. }
        | Cmd::Bridge
        | Cmd::Settings { .. }
        | Cmd::Profile { .. }
        | Cmd::Launch { .. } => {
            unreachable!("handled before connecting")
        }
    }
    Ok(())
}

async fn set_setting(
    proxy: &zbus::Proxy<'_>,
    (key, value): settings_cmd::Setting,
) -> Result<(), String> {
    proxy
        .call::<_, _, ()>("SetSetting", &(key, value.to_string(), ORIGIN))
        .await
        .map_err(describe)
}

async fn restore(proxy: &zbus::Proxy<'_>, cmd: RestoreCmd) -> Result<(), String> {
    match cmd {
        RestoreCmd::List => {
            let json: String = proxy
                .call("ListRestorePoints", &())
                .await
                .map_err(describe)?;
            settings_cmd::restore_lines(&json)?
                .iter()
                .for_each(|l| println!("{l}"));
        }
        RestoreCmd::Save { name } => {
            let id: String = proxy
                .call("SaveRestorePoint", &(name,))
                .await
                .map_err(describe)?;
            println!("{id}");
        }
        RestoreCmd::Delete { id } => proxy
            .call::<_, _, ()>("DeleteRestorePoint", &(id,))
            .await
            .map_err(describe)?,
        RestoreCmd::Restore(args) => match args.as_slice() {
            [id] => proxy
                .call::<_, _, ()>("Restore", &(id,))
                .await
                .map_err(describe)?,
            _ => return Err("usage: uqctl restore list|save <name>|delete <id>|<id>".into()),
        },
    }
    Ok(())
}

async fn current_pinned(proxy: &zbus::Proxy<'_>) -> Result<Vec<String>, String> {
    let json: String = proxy.call("GetSettings", &()).await.map_err(describe)?;
    let v: Value =
        serde_json::from_str(&json).map_err(|e| format!("bad settings from uqd: {e}"))?;
    Ok(settings_cmd::pinned_of(&v))
}

async fn pin(proxy: &zbus::Proxy<'_>, cmd: PinCmd) -> Result<(), String> {
    let current = current_pinned(proxy).await?;
    let next = match &cmd {
        PinCmd::List => {
            current.iter().for_each(|id| println!("{id}"));
            return Ok(());
        }
        PinCmd::Add { id } => settings_cmd::pinned_with(&current, id),
        PinCmd::Rm { id } => settings_cmd::pinned_without(&current, id),
    };
    match next {
        Some(list) => set_setting(proxy, ("pinned", serde_json::json!(list))).await,
        None => Ok(()), // already as asked
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Cmd::Setup { apps, json, quiet } = cli.cmd {
        // Local files only: no uqd, no timeout.
        return setup::main(apps, json, quiet);
    }
    if let Cmd::Settings { page, setting } = cli.cmd {
        return settings_app::main(
            page.as_deref().unwrap_or(""),
            setting.as_deref().unwrap_or(""),
        )
        .await;
    }
    if let Cmd::Launch { id } = &cli.cmd {
        // Bounded inside: uqd gets 500 ms, Hyprland its own request timeout.
        return launch_cmd::run(id).await;
    }
    if matches!(cli.cmd, Cmd::Bridge) {
        // Long-lived: no timeout; ends on stdin EOF.
        return bridge::run(tokio::io::stdin(), tokio::io::stdout()).await;
    }
    // uqd waits at most ~12 s on Hyprland (verify + reload); never hang a keybind forever.
    let res = tokio::time::timeout(std::time::Duration::from_secs(15), run(cli.cmd))
        .await
        .unwrap_or_else(|_| Err("uqd did not answer within 15 s".into()));
    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("uqctl: {msg}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_schedule_json() {
        let json = schedule_json(&ScheduleCmd::Fixed {
            light_at: "07:00".into(),
            dark_at: "19:30".into(),
        })
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["mode"], "fixed");
        assert_eq!(v["light_at"], "07:00:00");
        assert_eq!(v["dark_at"], "19:30:00");
    }

    #[test]
    fn sun_and_off_schedule_json() {
        let v: serde_json::Value = serde_json::from_str(
            &schedule_json(&ScheduleCmd::Sun {
                lat: 51.5,
                lon: -0.13,
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            (v["mode"].as_str(), v["lat"].as_f64()),
            (Some("sun"), Some(51.5))
        );
        assert_eq!(
            schedule_json(&ScheduleCmd::Off).unwrap(),
            r#"{"mode":"off"}"#
        );
    }

    #[test]
    fn active_workspace_selector_prefers_id() {
        let json = r#"{"id": 3, "name": "web", "monitor": "DP-1", "windows": 2}"#;
        assert_eq!(active_workspace_selector(json).unwrap(), "3");
        let special = r#"{"id": -98, "name": "special:magic"}"#;
        assert_eq!(active_workspace_selector(special).unwrap(), "special:magic");
        let named = r#"{"id": -1337, "name": "scratch"}"#;
        assert_eq!(active_workspace_selector(named).unwrap(), "name:scratch");
        assert!(active_workspace_selector("not json").is_err());
        assert!(active_workspace_selector(r#"{"name": 1}"#).is_err());
    }

    #[test]
    fn cli_parses_setup() {
        assert!(matches!(
            Cli::try_parse_from(["uqctl", "setup"]).unwrap().cmd,
            Cmd::Setup {
                apps: false,
                json: false,
                quiet: false
            }
        ));
        assert!(matches!(
            Cli::try_parse_from(["uqctl", "setup", "--apps"])
                .unwrap()
                .cmd,
            Cmd::Setup {
                apps: true,
                json: false,
                quiet: false
            }
        ));
        assert!(matches!(
            Cli::try_parse_from(["uqctl", "setup", "--quiet"])
                .unwrap()
                .cmd,
            Cmd::Setup { quiet: true, .. }
        ));
        assert!(Cli::try_parse_from(["uqctl", "setup", "--quiet", "--json"]).is_err());
    }

    #[test]
    fn bad_time_is_rejected() {
        let err = schedule_json(&ScheduleCmd::Fixed {
            light_at: "7am".into(),
            dark_at: "19:00".into(),
        })
        .unwrap_err();
        assert!(err.contains("7am"), "{err}");
    }

    #[test]
    fn cli_parses_theme_set() {
        let cli = Cli::try_parse_from(["uqctl", "theme", "set", "rosewood", "light"]).unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Theme { cmd: ThemeCmd::Set { ref id, ref variant } } if id == "rosewood" && variant == "light")
        );
    }
}
