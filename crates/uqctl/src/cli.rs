//! The command line: every subcommand and its flags (clap).

use super::*;

#[derive(Parser)]
#[command(name = "uqctl", about = "Control the uQ desktop", version)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) cmd: Cmd,
}

#[derive(Subcommand)]
pub(crate) enum Cmd {
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
    /// Keyboard shortcuts: list them, or put uQ's own back to their default keys.
    Keys {
        #[command(subcommand)]
        cmd: KeysCmd,
    },
    /// Open the Settings app, optionally on a page and a setting (e.g. `uqctl settings wallpaper`).
    Settings {
        page: Option<String>,
        setting: Option<String>,
    },
    /// Open Welcome, the first-run tour in Settings, if it hasn't been finished yet. Prints a
    /// note and does nothing otherwise.
    Welcome {
        /// Start Welcome over from its first step, even if it was finished.
        #[arg(long)]
        reset: bool,
    },
    /// The clipboard history kept by uqlip: list, copy again, pin, delete, pause.
    Clip {
        #[command(subcommand)]
        cmd: clip_cmd::ClipCmd,
    },
    /// Show the wallpaper, or set a picture as it (`uqctl wallpaper set -- PICTURE`).
    Wallpaper {
        #[command(subcommand)]
        cmd: wallpaper_cmd::WallpaperCmd,
    },
    /// uQ's warnings and errors from the journal, each with its code (`uqctl explain CODE`).
    Log(diag_cmd::LogArgs),
    /// What a problem code means and what to do about it (`uqctl explain HYPR-003`); every code
    /// without one.
    Explain { code: Option<String> },
    /// Crashes of the shell, the lock screen, uQ's programs and apps: list them, or dismiss one.
    Crashes {
        #[command(subcommand)]
        cmd: Option<diag_cmd::CrashesCmd>,
    },
    /// Make a report to paste into an issue, about a crash (`--crash latest`, or an id from
    /// `uqctl crashes`) or about now. Prints where it was saved. Window titles, names and
    /// addresses are taken out unless `--no-redact`.
    Report {
        #[arg(long)]
        crash: Option<String>,
        /// Print the report instead of saving it.
        #[arg(long)]
        stdout: bool,
        #[arg(long)]
        no_redact: bool,
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
    /// fix from Apps and scaling, in its own scope. `--terminal` starts the terminal chosen in
    /// Settings › Default apps instead (what Super+Return runs).
    Launch {
        #[arg(required_unless_present = "terminal")]
        id: Option<String>,
        /// Start the chosen terminal (else kyko, then kitty).
        #[arg(long, conflicts_with = "id")]
        terminal: bool,
    },
    /// Run the shell's JSON-lines bridge on stdin/stdout (started by the shell, not by hand).
    Bridge {
        /// Read widgets, app actions and providers from this directory instead of /usr/share/uq
        /// (laid out the same way).
        #[arg(long, hide = true, value_name = "DIR")]
        root: Option<std::path::PathBuf>,
    },
    /// Widgets, settings pages, launcher providers and app actions from other apps.
    Integrations {
        #[command(subcommand)]
        cmd: IntegrationsCmd,
    },
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
    /// Make greetd with uQ's login screen the display manager, or switch back (as root:
    /// `run0 uqctl greeter enable`).
    Greeter {
        #[command(subcommand)]
        cmd: greeter_cmd::GreeterCmd,
    },
    /// Packaging: render every theme variant's tokens.json into OUT/<id>-<variant>/ for the
    /// greeter, with uqd's renderer.
    #[command(hide = true)]
    RenderThemes(render_themes::Args),
}

#[derive(Subcommand)]
pub(crate) enum IntegrationsCmd {
    /// Everything uQ found under /usr/share/uq/: kind, id, shown/hidden/invalid, source and why.
    List {
        /// Print one JSON array instead.
        #[arg(long)]
        json: bool,
        /// Read this directory instead of /usr/share/uq (laid out the same way).
        #[arg(long, hide = true, value_name = "DIR")]
        root: Option<std::path::PathBuf>,
    },
    /// Check an app's uq/ folder before packaging. Exit 0: clean; 1: problems listed.
    Check {
        dir: std::path::PathBuf,
        /// Don't check that the programs commands and app actions run are installed (for the
        /// CI of an app whose programs aren't installed there). Paths are still checked.
        #[arg(long)]
        no_programs: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum LayoutCmd {
    Get,
    /// e.g. `uqctl layout set 2 scrolling`; WORKSPACE may be `current`.
    Set {
        workspace: String,
        layout: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum ScheduleCmd {
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
