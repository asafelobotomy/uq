//! The QML-facing `App` singleton: which page is shown, which row to highlight, whether uqd is up,
//! and the sidebar's pages.
//!
//! D-Bus work runs on a tokio thread (`crate::session`); it reaches `App` only through [`post`],
//! which queues the event onto the Qt thread, or keeps it until `App` exists.

use std::pin::Pin;
use std::sync::Mutex;

use cxx_qt::{CxxQtThread, Threading};
use cxx_qt_lib::QString;

use crate::registry;

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
        /// The setting id whose `SettingRow` flashes; set to "" first so a repeat flashes again.
        #[qproperty(QString, highlight)]
        /// Whether `org.uq.Desktop1` has an owner.
        #[qproperty(bool, uqd_up)]
        /// `[{id, name, icon, category, categoryName}]` in sidebar order.
        #[qproperty(QString, pages_json)]
        /// `--screenshots <dir>`: QML visits each page, saves it here, then quits.
        #[qproperty(QString, screenshot_dir)]
        type App = super::AppRust;

        /// Show `page` (unknown ids are logged and ignored) and highlight `setting`.
        #[qinvokable]
        fn show_page(self: Pin<&mut App>, page: &QString, setting: &QString);

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
    page: QString,
    highlight: QString,
    uqd_up: bool,
    pages_json: QString,
    screenshot_dir: QString,
}

impl Default for AppRust {
    fn default() -> Self {
        let first = registry::core_pages().first().map_or("", |p| p.id);
        Self {
            page: QString::from(first),
            highlight: QString::default(),
            uqd_up: false,
            pages_json: QString::from(&registry::pages_json()),
            screenshot_dir: QString::default(),
        }
    }
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
}

#[derive(Default)]
struct Inbox {
    qt: Option<CxxQtThread<App>>,
    pending: Vec<Event>,
}

static INBOX: Mutex<Inbox> = Mutex::new(Inbox {
    qt: None,
    pending: Vec::new(),
});

/// Hand `event` to `App` on the Qt thread; before `App` exists, keep it for [`App::initialize`].
pub fn post(event: Event) {
    let mut inbox = INBOX.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(qt) = &inbox.qt {
        let ev = event.clone();
        if qt.queue(move |app| app.apply(ev)).is_ok() {
            return;
        }
        inbox.qt = None; // the App is gone; nothing will read this
    }
    inbox.pending.push(event);
}

impl cxx_qt::Initialize for App {
    fn initialize(mut self: Pin<&mut Self>) {
        let mut inbox = INBOX.lock().unwrap_or_else(|e| e.into_inner());
        inbox.qt = Some(self.qt_thread());
        for event in std::mem::take(&mut inbox.pending) {
            self.as_mut().apply(event);
        }
    }
}

/// What a `ShowPage(page, setting)` does to the page: `None` keeps the current one.
pub fn target_page(page: &str) -> Option<&'static str> {
    if page.is_empty() {
        return None;
    }
    let found = registry::find(page).map(|p| p.id);
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
        }
    }

    fn show(mut self: Pin<&mut Self>, page: &str, setting: &str) {
        let Some(id) = target_page(page) else {
            return;
        };
        self.as_mut().set_page(QString::from(id));
        self.as_mut().set_highlight(QString::default());
        self.set_highlight(QString::from(setting));
    }

    fn show_page(self: Pin<&mut Self>, page: &QString, setting: &QString) {
        self.show(&page.to_string(), &setting.to_string());
    }

    fn grab_window(&self, path: &QString) -> bool {
        crate::window::ffi::uqs_grab_window(&path.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_page_keeps_current_for_empty_and_unknown() {
        assert_eq!(target_page(""), None);
        assert_eq!(target_page("nope"), None);
        assert_eq!(target_page("wallpaper"), Some("wallpaper"));
    }
}
