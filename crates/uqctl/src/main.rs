//! uqctl: command-line client for uqd (`org.uq.Desktop1`).

mod bridge;

use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::process::ExitCode;
use uq_core::paths::Paths;
use uq_core::schedule::Schedule;
use uq_core::stub::{StubOutcome, consumer_files, install_consumers, install_stub, stub_contents};

const NAME: &str = "org.uq.Desktop1";
const PATH: &str = "/org/uq/Desktop1";

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
    /// Re-render every config file from uqd's state.
    Reload,
    /// Run the shell's JSON-lines bridge on stdin/stdout (started by the shell, not by hand).
    Bridge,
    /// Install uQ's ~/.config/hypr/hyprland.lua (never overwrites an existing one).
    Setup {
        /// Also point GTK, qt6ct, kitty, foot and alacritty at uQ's theme. These files apply in
        /// every desktop you log in to, not only Hyprland.
        #[arg(long)]
        apps: bool,
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

fn setup(apps: bool) -> Result<(), String> {
    let paths = Paths::from_env();
    match install_stub(&paths).map_err(|e| format!("{}: {e}", paths.main_lua.display()))? {
        StubOutcome::Installed => {
            println!("uQ config installed at {}", paths.main_lua.display());
            println!("Log out and choose Hyprland, or run `hyprctl reload` in a running session.");
        }
        StubOutcome::AlreadyPresent => {
            println!(
                "{} already exists and was left alone.",
                paths.main_lua.display()
            );
            println!("To use uQ, put these lines at the top of it:\n");
            print!("{}", stub_contents(&paths));
        }
    }
    if !apps {
        println!(
            "App theming (GTK, qt6ct, terminals) not changed. Run `uqctl setup --apps` to opt in; \
             those files also apply in other desktops."
        );
        return Ok(());
    }
    let consumers = install_consumers(&paths).map_err(|e| format!("app theme files: {e}"))?;
    let lines: HashMap<_, _> = consumer_files(&paths).into_iter().collect();
    for (path, outcome) in consumers {
        match outcome {
            StubOutcome::Installed => println!("themed: {}", path.display()),
            StubOutcome::AlreadyPresent => println!(
                "left alone: {} (add this to use uQ's theme):\n  {}",
                path.display(),
                lines[&path].trim_end().replace('\n', "\n  ")
            ),
        }
    }
    Ok(())
}

fn describe(err: zbus::Error) -> String {
    match err {
        zbus::Error::MethodError(_, Some(msg), _) => msg,
        zbus::Error::MethodError(name, None, _) => name.to_string(),
        other => format!("{other} (is uqd running?)"),
    }
}

async fn run(cmd: Cmd) -> Result<(), String> {
    if let Cmd::Setup { apps } = cmd {
        return setup(apps);
    }
    if matches!(cmd, Cmd::Bridge) {
        unreachable!("handled in main");
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
                .call::<_, _, ()>("SetTheme", &(id, variant))
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
                .call::<_, _, ()>("SetLayout", &(workspace, layout))
                .await
                .map_err(describe)?;
        }
        Cmd::Schedule { cmd } => {
            proxy
                .call::<_, _, ()>("SetSchedule", &(schedule_json(&cmd)?,))
                .await
                .map_err(describe)?;
        }
        Cmd::Reload => proxy
            .call::<_, _, ()>("Reload", &())
            .await
            .map_err(describe)?,
        Cmd::Setup { .. } | Cmd::Bridge => unreachable!("handled before connecting"),
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse();
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
            Cmd::Setup { apps: false }
        ));
        assert!(matches!(
            Cli::try_parse_from(["uqctl", "setup", "--apps"])
                .unwrap()
                .cmd,
            Cmd::Setup { apps: true }
        ));
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
