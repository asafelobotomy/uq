//! uQ's Settings app: a Kirigami window with a category sidebar and one page at a time.
//!
//! Single instance (spec §2.1): the first process owns `org.uq.Settings1`; a later
//! `uq-settings --page P --setting S` forwards `ShowPage(P, S)` to it and exits 0.
//!
//! `uq-settings --screenshots <dir>` renders every page offscreen into `<dir>/<page>.png` and
//! exits, without touching the session bus. (Test aid: with `UQ_SETTINGS_SHOTS_BUS` and an explicit
//! `DBUS_SESSION_BUS_ADDRESS` it also follows uqd on that bus, to draw the pages with data.)

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use cxx_qt_lib::{QQuickStyle, QString};

mod app;
mod appearance_model;
mod desktop_entries;
mod displays_model;
mod history_model;
mod input_model;
mod instance;
mod look_model;
mod model_core;
mod pages;
mod plugins;
mod registry;
mod search;
mod session;
mod shell_model;
mod system_lists;
mod wallpaper_model;
mod window;
mod xkb_lists;

use app::Event;
use session::StartError;

#[derive(Parser)]
#[command(version, about = "uQ Settings")]
struct Args {
    /// The page to show (e.g. appearance, wallpaper).
    #[arg(long, default_value = "")]
    page: String,
    /// A setting on that page to highlight.
    #[arg(long, default_value = "")]
    setting: String,
    /// Render every page into this directory, then quit.
    #[arg(long, hide = true)]
    screenshots: Option<PathBuf>,
    /// Read plugin pages from this directory instead of /usr/share/uq/settings/pages.
    #[arg(long, hide = true, value_name = "DIR")]
    plugin_dir: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    if let Some(dir) = args.plugin_dir.clone() {
        plugins::set_dir(dir);
    }
    if args.screenshots.is_some() && std::env::var_os("QT_QPA_PLATFORM").is_none() {
        // SAFETY: single-threaded here, before Qt, tokio or any other thread starts.
        unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };
    }

    // Queued before the bus is up, so a forwarded ShowPage lands after it.
    app::post(Event::Show {
        page: args.page.clone(),
        setting: args.setting.clone(),
    });

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("uq-settings: can't start the D-Bus runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    let _session = match &args.screenshots {
        Some(dir) => {
            app::post(Event::ScreenshotDir(dir.display().to_string()));
            // Only against a private bus the caller points us at; never the real session.
            if std::env::var_os("UQ_SETTINGS_SHOTS_BUS").is_none() {
                None
            } else if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
                eprintln!(
                    "uq-settings: UQ_SETTINGS_SHOTS_BUS needs DBUS_SESSION_BUS_ADDRESS set to a private bus; not connecting"
                );
                None
            } else {
                runtime
                    .block_on(session::follow_only())
                    .map_err(|e| {
                        eprintln!("uq-settings: can't follow uqd for the screenshots: {e}")
                    })
                    .ok()
            }
        }
        None => match runtime.block_on(session::start(&args.page, &args.setting)) {
            Ok(Some(session)) => Some(session),
            Ok(None) => return ExitCode::SUCCESS, // the running instance shows the page
            Err(StartError::NoBus(e)) => {
                eprintln!("uq-settings: no session bus ({e}); running without single instance");
                None
            }
            Err(StartError::Claim(e)) => {
                eprintln!("uq-settings: couldn't claim or reach org.uq.Settings1: {e}");
                return ExitCode::FAILURE;
            }
        },
    };

    cxx_qt::init_qml_module!("org.uq.settings");
    // The KDE desktop style draws through the Qt widget style, so Kvantum themes it.
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        QQuickStyle::set_style(&QString::from("org.kde.desktop"));
    }

    match window::ffi::uqs_run(
        env!("CARGO_PKG_VERSION"),
        "qrc:/qt/qml/org/uq/settings/qml/Main.qml",
    ) {
        0 => ExitCode::SUCCESS,
        -1 => {
            eprintln!("uq-settings: the window couldn't be created (is Kirigami installed?)");
            ExitCode::FAILURE
        }
        _ => ExitCode::FAILURE,
    }
}
