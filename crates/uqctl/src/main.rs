//! uqctl: command-line client for uqd (`org.uq.Desktop1`).

use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::process::ExitCode;
use uq_core::schedule::Schedule;

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
    /// e.g. `uqctl layout set 2 scrolling`
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

fn describe(err: zbus::Error) -> String {
    match err {
        zbus::Error::MethodError(_, Some(msg), _) => msg,
        zbus::Error::MethodError(name, None, _) => name.to_string(),
        other => format!("{other} (is uqd running?)"),
    }
}

async fn run(cmd: Cmd) -> Result<(), String> {
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
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.cmd).await {
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
