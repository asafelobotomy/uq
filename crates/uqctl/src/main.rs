//! uqctl: command-line client for uqd (`org.uq.Desktop1`).

mod bridge;
mod clip_cmd;
mod diag_cmd;
mod greeter_cmd;
mod integrations;
mod keys_cmd;
mod launch_cmd;
mod profile_cmd;
mod render_themes;
mod reset_cmd;
mod settings_app;
mod settings_cmd;
mod setup;
mod theme_cmd;
mod wallpaper_cmd;
mod welcome_cmd;

use clap::{Parser, Subcommand};
use keys_cmd::KeysCmd;
use profile_cmd::ProfileCmd;
use serde_json::Value;
use settings_cmd::{
    IdleCmd, LaunchesCmd, MouseCmd, NotifCmd, OnOff, PinCmd, PrivacyKey, RestoreCmd, WeatherCmd,
};
use std::collections::HashMap;
use std::process::ExitCode;
use theme_cmd::ThemeCmd;
use uq_core::schedule::Schedule;
mod cli;
use cli::*;
mod desktop_cmd;
use desktop_cmd::*;

const NAME: &str = "org.uq.Desktop1";
const PATH: &str = "/org/uq/Desktop1";
/// The origin uqd's history records for uqctl's writes.
const ORIGIN: &str = "uqctl";

/// `uqctl history | head` closes stdout early; `println!` then panics. Treat that like SIGPIPE
/// would: quietly stop (exit 0). Not for `bridge`, which writes through tokio and ends on its
/// own when the shell goes away.
fn quiet_broken_pipe() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = info
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| info.payload().downcast_ref::<&str>().copied())
            .unwrap_or_default();
        if msg.starts_with("failed printing to std") && msg.contains("Broken pipe") {
            std::process::exit(0);
        }
        default(info);
    }));
}

/// Ask the kernel to send SIGTERM when the process that started us exits, even on SIGKILL or a
/// crash. The shell runs `bridge` and `clip watch` for as long as it is up, and uq-shell.service
/// keeps `KillMode=process` (apps the shell opened outlive a restart), so these have to end
/// themselves. Doing it here saves the shell an extra `setpriv` exec per helper.
fn end_with_parent() {
    // SAFETY: getppid and prctl(PR_SET_PDEATHSIG) take no pointers and can't fail in a way
    // that matters here (an unsupported kernel just leaves the old behaviour).
    unsafe {
        let parent = libc::getppid();
        libc::prctl(
            libc::PR_SET_PDEATHSIG,
            libc::SIGTERM as libc::c_ulong,
            0,
            0,
            0,
        );
        // The parent may have gone between our start and the prctl: then nobody will signal.
        if libc::getppid() != parent {
            std::process::exit(0);
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    // uqctl's own messages are for the person at the terminal; only a crash is recorded.
    uq_core::diag::log::install_panic_hook("uqctl");
    let cli = Cli::parse();
    if matches!(
        cli.cmd,
        Cmd::Bridge { .. }
            | Cmd::Clip {
                cmd: clip_cmd::ClipCmd::Watch
            }
    ) {
        end_with_parent();
    }
    if !matches!(cli.cmd, Cmd::Bridge { .. }) {
        quiet_broken_pipe();
    }
    if let Cmd::Setup { apps, json, quiet } = cli.cmd {
        // Local files only: no uqd, no timeout.
        return setup::main(apps, json, quiet);
    }
    if let Cmd::Greeter {
        cmd: greeter_cmd::GreeterCmd::SyncTheme { theme, variant },
    } = &cli.cmd
    {
        return greeter_cmd::sync_theme(theme, variant).await;
    }
    if let Cmd::Greeter { cmd } = &cli.cmd {
        // Local files and systemctl only: no uqd.
        return greeter_cmd::main(cmd);
    }
    if let Cmd::RenderThemes(args) = cli.cmd {
        return render_themes::main(&args);
    }
    if let Cmd::Integrations { cmd } = cli.cmd {
        // Local files, plus a bounded look at the bus for `requires.bus_name`.
        return match cmd {
            IntegrationsCmd::List { json, root } => integrations::list(json, root).await,
            IntegrationsCmd::Check { dir, no_programs } => integrations::check(&dir, !no_programs),
        };
    }
    if let Cmd::Settings { page, setting } = cli.cmd {
        return settings_app::main(
            page.as_deref().unwrap_or(""),
            setting.as_deref().unwrap_or(""),
        )
        .await;
    }
    if let Cmd::Welcome { reset } = cli.cmd {
        return welcome_cmd::main(reset).await;
    }
    match cli.cmd {
        Cmd::Log(a) => return diag_cmd::log(a),
        Cmd::Explain { code } => return diag_cmd::explain(code),
        Cmd::Crashes { cmd } => return diag_cmd::crashes(cmd),
        Cmd::Report {
            crash,
            stdout,
            no_redact,
        } => return diag_cmd::report(crash, stdout, no_redact),
        _ => {}
    }
    if let Cmd::Clip { cmd } = cli.cmd {
        // uqlip's own socket, not the desktop bus.
        return clip_cmd::main(cmd);
    }
    if let Cmd::Launch { id, terminal } = &cli.cmd {
        // Bounded inside: uqd gets 500 ms, Hyprland its own request timeout.
        let target = match id {
            Some(id) if !terminal => launch_cmd::Target::App(id.clone()),
            _ => launch_cmd::Target::Terminal,
        };
        return launch_cmd::run(&target).await;
    }
    if let Cmd::Bridge { root } = &cli.cmd {
        let roots = root
            .as_deref()
            .map_or_else(bridge::IntegrationRoots::system, |r| {
                integrations::roots_under(r).into()
            });
        // Long-lived: no timeout; ends on stdin EOF.
        // stdin and stdout each on one thread of their own, not tokio's blocking pool.
        return match (
            bridge::stdio::ThreadStdin::spawn(),
            bridge::stdio::ThreadStdout::spawn(),
        ) {
            (Ok(stdin), Ok(stdout)) => bridge::run(stdin, stdout, roots).await,
            (Err(e), _) | (_, Err(e)) => {
                eprintln!("uqctl bridge: cannot start the stdio threads: {e}");
                ExitCode::FAILURE
            }
        };
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
mod tests;
