//! `uqctl bridge`: the shell's long-lived JSON-lines child process (spec §2.3).
//!
//! stdout is protocol only; everything else goes to stderr. One task owns stdout and receives
//! `Outbound` over an mpsc channel, so concurrent replies and events never interleave mid-line.

mod appactions;
mod calc;
mod contents;
pub(crate) mod defaultapps;
pub(crate) mod desktop_entry;
mod dispatch;
mod feeds;
mod files;
pub(crate) mod launch;
mod locknotify;
mod logind;
pub(crate) mod net;
mod news;
mod notifowner;
mod opener;
mod power;
mod procusage;
pub mod protocol;
mod providers;
pub(crate) mod run;
mod settings_search;
mod staleapps;
pub(crate) mod stdio;
mod stocks;
mod sysmon;
pub(crate) mod uqd;
mod web;
mod welcome;
mod widgets;
mod worldclock;

use protocol::{Inbound, MAX_LINE, Outbound, Rejected, encode, read_lines};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{Semaphore, mpsc, watch};
use tokio_util::sync::CancellationToken;
mod serve;
pub use serve::*;

/// Requests in flight at once; the reader waits for a slot, so a flood cannot grow tasks forever.
const MAX_IN_FLIGHT: usize = 64;
/// How long the writer may take to drain queued output after stdin EOF.
const DRAIN: Duration = Duration::from_secs(2);

/// Where the integration files are read from (Phase 4 spec §2.2, §2.4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationRoots {
    /// `/usr/share/uq/shell/widgets`.
    pub widgets: PathBuf,
    /// `/usr/share/uq/launcher/actions`.
    pub actions: PathBuf,
    /// Launcher provider directories (system, then the user's).
    pub providers: Vec<PathBuf>,
    /// Where `[requires] bins` must exist.
    pub bin_dir: PathBuf,
}

impl IntegrationRoots {
    /// What an installed uQ reads.
    pub fn system() -> IntegrationRoots {
        IntegrationRoots::from(uq_core::integration::scan::Roots::system())
    }
}

impl From<uq_core::integration::scan::Roots> for IntegrationRoots {
    fn from(r: uq_core::integration::scan::Roots) -> IntegrationRoots {
        let mut providers = r.providers;
        // The user's provider directory stays, as Phase 2 defined it.
        for d in uq_core::launcher_providers::default_dirs()
            .into_iter()
            .skip(1)
        {
            if !providers.contains(&d) {
                providers.push(d);
            }
        }
        IntegrationRoots {
            widgets: r.widgets,
            actions: r.actions,
            providers,
            bin_dir: r.bin_dir,
        }
    }
}

/// What a request may use; cloned into each request task.
#[derive(Clone)]
struct Services {
    link: Arc<uqd::Link>,
    net: Arc<net::Net>,
    stocks: Arc<stocks::StocksLoop>,
    news: Arc<news::NewsLoop>,
    runs: Arc<Mutex<run::RunState>>,
    providers: Arc<providers::Providers>,
    logind: Arc<logind::Logind>,
    /// The shell's idle state, for logind's `SetIdleHint` (`Logind::idle_loop`).
    idle: Arc<watch::Sender<Option<bool>>>,
    procs: Arc<procusage::ProcUsage>,
    /// None when the bridge runs without integrations (unit tests).
    widgets: Option<Arc<widgets::Widgets>>,
    actions: Option<Arc<appactions::AppActions>>,
}

const NO_INTEGRATIONS: &str = "integrations are off";

/// Handle one request. This is the single dispatch point later tasks extend with new calls;
/// anything not matched is an error reply.
async fn handle(s: &Services, call: &str, args: &[Value]) -> Result<Value, String> {
    let (link, runs) = (&*s.link, &*s.runs);
    match call {
        "Ping" => Ok(json!("pong")),
        "GetTheme" | "SetTheme" | "ListThemes" | "ListThemeInfo" | "GetLayouts" | "SetLayout"
        | "GetSchedule" | "SetSchedule" | "Reload" | "GetSettings" | "SetSetting"
        | "GetLaunchCounts" | "RecordLaunch" | "ListProfiles" | "ApplyProfile" | "ProfileOff"
        | "ListOutputs" | "PickDone" | "SetBarLayout" => link.call(call, args).await,
        "Launch" if args.first().is_some_and(|a| a.get("app_action").is_some()) => {
            match &s.actions {
                Some(a) => a.launch(link, args).await,
                None => Err(NO_INTEGRATIONS.into()),
            }
        }
        "WelcomeStatus" if args.is_empty() => {
            Ok(welcome::status(link, welcome::runtime_dir().as_deref(), None).await)
        }
        "WelcomeStatus" => Err("WelcomeStatus takes no arguments".into()),
        "Launch" => launch::launch(link, args, runs).await,
        "Search" => s.providers.search(link, runs, args).await,
        "Activate" => s.providers.activate(link, runs, args).await,
        "LauncherOpened" => s.providers.launcher_opened(link, args).await,
        "WidgetShown" => s.widgets.as_ref().ok_or(NO_INTEGRATIONS)?.set_shown(args),
        "WidgetCall" | "WidgetGet" | "WidgetRun" => {
            let w = s.widgets.as_ref().ok_or(NO_INTEGRATIONS)?;
            match call {
                "WidgetCall" => w.call(args).await,
                "WidgetGet" => w.get(args).await,
                _ => w.run_command(args).await,
            }
        }
        "Dispatch" => dispatch::dispatch(args).await,
        "WindowLook" if args.is_empty() => dispatch::window_look().await,
        "WindowLook" => Err("WindowLook takes no arguments".into()),
        "DefaultApps" => defaultapps::default_apps(link, args),
        "StaleApps" => staleapps::stale_apps(args),
        "Power" => power::power(args).await,
        "Lock" => logind::lock(args).await,
        "SleepLockReady" if args.is_empty() => {
            s.logind.sleep_lock_ready();
            Ok(Value::Null)
        }
        "SleepLockReady" => Err("SleepLockReady takes no arguments".into()),
        "IdleHint" => match args {
            [Value::Bool(idle)] => {
                s.idle.send_replace(Some(*idle));
                Ok(Value::Null)
            }
            _ => Err("IdleHint takes one boolean argument".into()),
        },
        "GeocodeCity" => match args {
            [Value::String(name)] => s.net.geocode(name).await,
            _ => Err("GeocodeCity takes one string argument (the city name)".into()),
        },
        "NetworkChanged" => match args {
            [Value::Object(o)] if o.get("vpn_active").is_some_and(Value::is_boolean) => {
                s.stocks.network_changed();
                s.news.network_changed();
                s.net.network_changed(o["vpn_active"] == true).await;
                Ok(Value::Null)
            }
            _ => Err("NetworkChanged takes {vpn_active: bool}".into()),
        },
        "RefreshStocks" if args.is_empty() => Ok(json!(s.stocks.refresh_now())),
        "RefreshNews" if args.is_empty() => Ok(json!(s.news.refresh_now())),
        "RefreshStocks" | "RefreshNews" => Err(format!("{call} takes no arguments")),
        "ProcessUsage" => {
            let pids = procusage::pids_arg(args)?;
            Ok(s.procs.usage(&pids, std::time::Instant::now()))
        }
        "OpenLink" => match args {
            [Value::String(url)] => s.news.src.open_link(url),
            _ => Err("OpenLink takes one string argument (the link)".into()),
        },
        "PreviewFeed" => match args {
            [Value::String(url)] => s.news.src.preview(url).await,
            _ => Err("PreviewFeed takes one string argument (the feed address)".into()),
        },
        other => Err(format!("unknown call {other}")),
    }
}

/// Run the bridge on the real buses. `roots`: where integrations are read from.
pub async fn run<R, W>(stdin: R, stdout: W, roots: IntegrationRoots) -> ExitCode
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let locks = locknotify::spawn(std::env::var_os("WAYLAND_DISPLAY"));
    run_with_bus(stdin, stdout, None, None, locks, Some(roots)).await
}

#[cfg(test)]
mod tests;
