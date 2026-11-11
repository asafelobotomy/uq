//! The QML-facing `App` singleton: which page is shown, which row to highlight, whether uqd is up,
//! and the sidebar's pages.
//!
//! D-Bus work runs on a tokio thread (`crate::session`); it reaches `App` only through [`post`],
//! which queues the event onto the Qt thread, or keeps it until `App` exists.

use std::pin::Pin;
use std::sync::Mutex;

use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;

use crate::registry::{self, PageEntry};
use crate::{plugins, search};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        /// The id of the page on show.
        #[qproperty(QString, page)]
        /// A `ShowPage` (or `--page`) named a real page: it wins over the page the window
        /// remembered from last time.
        #[qproperty(bool, page_requested)]
        /// The setting id whose `SettingRow` flashes; set to "" first so a repeat flashes again.
        #[qproperty(QString, highlight)]
        /// Whether `org.uq.Desktop1` has an owner.
        #[qproperty(bool, uqd_up)]
        /// `[{id, name, icon, category, categoryName, qml, plugin}]` in sidebar order.
        #[qproperty(QString, pages_json)]
        /// The current design tokens (`tokens.json` as written by uqd), read once at startup;
        /// `{}` when missing or invalid.
        #[qproperty(QString, theme_json)]
        /// `--screenshots <dir>`: QML visits each page, saves it here, then quits.
        #[qproperty(QString, screenshot_dir)]
        /// Welcome (Phase 5 spec §5.1) is on show in place of the pages: `--welcome`,
        /// `ShowPage("welcome", "")` or Settings → About → Show Welcome.
        #[qproperty(bool, welcome)]
        /// Welcome shows every step, not only those added since the user last saw it.
        #[qproperty(bool, welcome_full)]
        type App = super::AppRust;

        /// Show `page` (unknown ids are logged and ignored) and highlight `setting`.
        #[qinvokable]
        fn show_page(self: Pin<&mut App>, page: &QString, setting: &QString);

        /// Search the pages' labels and keywords: `[{pageId, settingId, label, pageName, score}]`,
        /// best first; "" for an empty query.
        #[qinvokable]
        fn search(self: &App, query: &QString) -> QString;

        /// Show Welcome from its first step, with every step (Settings → About).
        #[qinvokable]
        fn show_welcome(self: Pin<&mut App>);

        /// Leave Welcome for the pages.
        #[qinvokable]
        fn close_welcome(self: Pin<&mut App>);

        /// Save the window as a PNG (`--screenshots`).
        #[qinvokable]
        fn grab_window(self: &App, path: &QString) -> bool;

        /// A `ShowPage` call arrived: the window should come to the front.
        #[qsignal]
        fn raise_requested(self: Pin<&mut App>);
    }

    impl cxx_qt::Threading for App {}
    impl cxx_qt::Initialize for App {}
}

use qobject::App;

pub struct AppRust {
    /// Core pages, then the plugin pages on show (updated as pages come and go).
    entries: Vec<PageEntry>,
    page: QString,
    page_requested: bool,
    highlight: QString,
    uqd_up: bool,
    pages_json: QString,
    screenshot_dir: QString,
    theme_json: QString,
    welcome: bool,
    welcome_full: bool,
}

impl Default for AppRust {
    fn default() -> Self {
        let mut entries = registry::all_pages(&plugins::scan_configured());
        registry::drop_unavailable(
            &mut entries,
            crate::pages::login_screen::greeter_installed(),
        );
        let first = entries.first().map_or("", |p| p.id.as_str());
        Self {
            page: QString::from(first),
            page_requested: false,
            pages_json: QString::from(&registry::pages_json(&entries)),
            entries,
            highlight: QString::default(),
            uqd_up: false,
            screenshot_dir: QString::default(),
            theme_json: QString::from(&read_tokens()),
            welcome: false,
            welcome_full: false,
        }
    }
}

/// `<theme_dir>/tokens.json` when it is a JSON object under 1 MiB, else `{}`.
fn read_tokens() -> String {
    let path = uq_core::paths::Paths::from_env()
        .theme_dir
        .join("tokens.json");
    std::fs::read(&path)
        .ok()
        .filter(|b| b.len() <= 1 << 20)
        .and_then(|b| String::from_utf8(b).ok())
        .filter(|t| matches!(serde_json::from_str::<serde_json::Value>(t), Ok(v) if v.is_object()))
        .unwrap_or_else(|| "{}".into())
}

/// What the D-Bus side tells the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// `ShowPage` (or the command line): show the page, highlight the row, raise the window.
    Show {
        page: String,
        setting: String,
    },
    UqdUp(bool),
    ScreenshotDir(String),
    /// The plugin pages to show changed (one was installed or removed, or the bus name it
    /// waits for came or went).
    Plugins(Vec<plugins::PluginPage>),
}

/// Where events go: kept until `App` exists, then queued to it, then dropped once it is gone.
enum Route<T> {
    Waiting(Vec<Event>),
    Live(T),
    Gone,
}

impl<T> Route<T> {
    /// Deliver or keep `event`; `send` returns false when the target is gone.
    fn post(&mut self, event: Event, send: impl FnOnce(&T, Event) -> bool) {
        match self {
            Route::Waiting(pending) => pending.push(event),
            Route::Live(target) => {
                if !send(target, event) {
                    eprintln!("uq-settings: the window is gone; dropping D-Bus events");
                    *self = Route::Gone;
                }
            }
            Route::Gone => {}
        }
    }

    /// The target exists: return what was kept for it.
    fn go_live(&mut self, target: T) -> Vec<Event> {
        match std::mem::replace(self, Route::Live(target)) {
            Route::Waiting(pending) => pending,
            _ => Vec::new(),
        }
    }
}

static INBOX: Mutex<Route<CxxQtThread<App>>> = Mutex::new(Route::Waiting(Vec::new()));

/// Hand `event` to `App` on the Qt thread; before `App` exists, keep it for [`App::initialize`].
pub fn post(event: Event) {
    let mut inbox = INBOX.lock().unwrap_or_else(|e| e.into_inner());
    inbox.post(event, |qt, ev| qt.queue(move |app| app.apply(ev)).is_ok());
}

impl cxx_qt::Initialize for App {
    fn initialize(mut self: Pin<&mut Self>) {
        let mut inbox = INBOX.lock().unwrap_or_else(|e| e.into_inner());
        for event in inbox.go_live(self.qt_thread()) {
            self.as_mut().apply(event);
        }
    }
}

/// The `ShowPage` page id that opens Welcome (`uqctl welcome`, `uq-settings --welcome`). It is
/// not a sidebar page.
pub const WELCOME_PAGE: &str = "welcome";

/// What a `ShowPage(page, setting)` does to the page: `None` keeps the current one.
pub fn target_page<'a>(entries: &'a [PageEntry], page: &str) -> Option<&'a str> {
    if page.is_empty() {
        return None;
    }
    let found = entries.iter().find(|p| p.id == page).map(|p| p.id.as_str());
    if found.is_none() {
        eprintln!("uq-settings: no page {page:?}; staying on the current one");
    }
    found
}

impl App {
    fn apply(mut self: Pin<&mut Self>, event: Event) {
        match event {
            Event::Show { page, setting } => {
                self.as_mut().show(&page, &setting);
                self.raise_requested();
            }
            Event::UqdUp(up) => self.set_uqd_up(up),
            Event::ScreenshotDir(dir) => self.set_screenshot_dir(QString::from(&dir)),
            Event::Plugins(pages) => self.set_plugins(&pages),
        }
    }

    /// Replace the plugin pages in the sidebar and the search. If the page on show went away,
    /// show the first page instead.
    fn set_plugins(mut self: Pin<&mut Self>, pages: &[plugins::PluginPage]) {
        let entries = registry::all_pages(pages);
        let json = QString::from(&registry::pages_json(&entries));
        let current = self.page().to_string();
        let first = entries.first().map(|p| p.id.clone()).unwrap_or_default();
        let gone = !entries.iter().any(|p| p.id == current);
        self.as_mut().rust_mut().get_mut().entries = entries;
        self.as_mut().set_pages_json(json);
        if gone {
            self.set_page(QString::from(&first));
        }
    }

    fn show(mut self: Pin<&mut Self>, page: &str, setting: &str) {
        if page == WELCOME_PAGE {
            // Already on show: keep the step the user is on.
            if !*self.welcome() {
                self.as_mut().set_welcome_full(false);
                self.set_welcome(true);
            }
            return;
        }
        let Some(id) = target_page(&self.entries, page).map(str::to_owned) else {
            return;
        };
        self.as_mut().set_page(QString::from(&id));
        self.as_mut().set_page_requested(true);
        self.as_mut().set_highlight(QString::default());
        self.set_highlight(QString::from(setting));
    }

    fn show_page(self: Pin<&mut Self>, page: &QString, setting: &QString) {
        self.show(&page.to_string(), &setting.to_string());
    }

    fn search(&self, query: &QString) -> QString {
        let hits = search::search(&self.entries, &query.to_string());
        QString::from(&serde_json::to_string(&hits).unwrap_or_else(|_| "[]".into()))
    }

    fn show_welcome(mut self: Pin<&mut Self>) {
        // Leave and come back, so the flow starts over at its first step.
        self.as_mut().set_welcome(false);
        self.as_mut().set_welcome_full(true);
        self.set_welcome(true);
    }

    fn close_welcome(self: Pin<&mut Self>) {
        self.set_welcome(false);
    }

    fn grab_window(&self, path: &QString) -> bool {
        crate::window::ffi::uqs_grab_window(&path.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_keeps_then_sends_then_drops() {
        let ev = |n: bool| Event::UqdUp(n);
        let mut route: Route<()> = Route::Waiting(Vec::new());
        route.post(ev(true), |_, _| panic!("not live yet"));
        assert_eq!(route.go_live(()), [ev(true)]);

        let mut sent = Vec::new();
        route.post(ev(false), |_, e| {
            sent.push(e);
            true
        });
        assert_eq!(sent, [ev(false)]);

        route.post(ev(true), |_, _| false); // the App is gone
        assert!(matches!(route, Route::Gone));
        route.post(ev(true), |_, _| panic!("gone: dropped, not sent"));
        assert!(matches!(route, Route::Gone));
    }

    #[test]
    fn target_page_keeps_current_for_empty_and_unknown() {
        let entries = registry::all_pages(&[]);
        assert_eq!(target_page(&entries, ""), None);
        assert_eq!(target_page(&entries, "nope"), None);
        assert_eq!(target_page(&entries, "wallpaper"), Some("wallpaper"));
    }
}
