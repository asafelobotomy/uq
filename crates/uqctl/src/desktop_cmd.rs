//! The commands that talk to uqd over `org.uq.Desktop1`: themes, layouts, schedule, history,
//! settings and pins.

use super::*;

pub(crate) fn parse_time(s: &str) -> Result<chrono::NaiveTime, String> {
    chrono::NaiveTime::parse_from_str(s, "%H:%M")
        .map_err(|e| format!("{s:?} is not a HH:MM time ({e})"))
}

/// The JSON form `SetSchedule` takes.
pub(crate) fn schedule_json(cmd: &ScheduleCmd) -> Result<String, String> {
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

/// A Hyprland workspace selector for the `j/activeworkspace` reply: the numeric id for
/// normal workspaces, otherwise `special:…` or `name:<name>` (a bare name never matches a rule).
pub(crate) fn active_workspace_selector(json: &str) -> Result<String, String> {
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

/// Asked on Hyprland's request socket directly (what `hyprctl -j activeworkspace` does), so a
/// layout keybind doesn't start a `hyprctl` process each time.
pub(crate) async fn current_workspace() -> Result<String, String> {
    let reply = uqd::hypr::HyprSocket::from_env()
        .request("j/activeworkspace")
        .await
        .map_err(|e| format!("asking Hyprland for the active workspace: {e}"))?;
    active_workspace_selector(&reply)
}

pub(crate) fn describe(err: zbus::Error) -> String {
    match err {
        zbus::Error::MethodError(_, Some(msg), _) => msg,
        zbus::Error::MethodError(name, None, _) => name.to_string(),
        other => format!("{other} (is uqd running?)"),
    }
}

pub(crate) async fn run(cmd: Cmd) -> Result<(), String> {
    if matches!(cmd, Cmd::Bridge { .. } | Cmd::Settings { .. }) {
        unreachable!("handled in main");
    }
    if let Cmd::Profile { cmd } = cmd {
        return profile_cmd::run(&profile_cmd::connect().await?, cmd).await;
    }
    if let Cmd::Keys { cmd } = cmd {
        let lines = keys_cmd::run(&profile_cmd::connect().await?, cmd).await?;
        lines.iter().for_each(|l| println!("{l}"));
        return Ok(());
    }
    if let Cmd::Theme { cmd } = cmd {
        return theme_cmd::run(&profile_cmd::connect().await?, cmd).await;
    }
    if let Cmd::Wallpaper { cmd } = cmd {
        let lines = wallpaper_cmd::run(&profile_cmd::connect().await?, cmd).await?;
        lines.iter().for_each(|l| println!("{l}"));
        return Ok(());
    }
    let conn = zbus::Connection::session().await.map_err(describe)?;
    let proxy = zbus::Proxy::new(&conn, NAME, PATH, NAME)
        .await
        .map_err(describe)?;
    match cmd {
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
                current_workspace().await?
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
        | Cmd::RenderThemes(_)
        | Cmd::Greeter { .. }
        | Cmd::Integrations { .. }
        | Cmd::Bridge { .. }
        | Cmd::Settings { .. }
        | Cmd::Welcome { .. }
        | Cmd::Log(_)
        | Cmd::Explain { .. }
        | Cmd::Crashes { .. }
        | Cmd::Report { .. }
        | Cmd::Clip { .. }
        | Cmd::Profile { .. }
        | Cmd::Keys { .. }
        | Cmd::Theme { .. }
        | Cmd::Wallpaper { .. }
        | Cmd::Launch { .. } => {
            unreachable!("handled before connecting")
        }
    }
    Ok(())
}

pub(crate) async fn set_setting(
    proxy: &zbus::Proxy<'_>,
    (key, value): settings_cmd::Setting,
) -> Result<(), String> {
    proxy
        .call::<_, _, ()>("SetSetting", &(key, value.to_string(), ORIGIN))
        .await
        .map_err(describe)
}

pub(crate) async fn restore(proxy: &zbus::Proxy<'_>, cmd: RestoreCmd) -> Result<(), String> {
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

pub(crate) async fn current_pinned(proxy: &zbus::Proxy<'_>) -> Result<Vec<String>, String> {
    let json: String = proxy.call("GetSettings", &()).await.map_err(describe)?;
    let v: Value =
        serde_json::from_str(&json).map_err(|e| format!("bad settings from uqd: {e}"))?;
    Ok(settings_cmd::pinned_of(&v))
}

pub(crate) async fn pin(proxy: &zbus::Proxy<'_>, cmd: PinCmd) -> Result<(), String> {
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
