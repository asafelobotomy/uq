//! The Wallpaper page's QML object. It runs [`WallpaperState`] operations on the tokio runtime
//! (never on the Qt thread) and queues each result back.
//!
//! Like `appearance_model.rs`, this file sits in `src/` because cxx-qt-build supports one
//! directory of bridge files per QML module (QTBUG-93443).

use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;
use futures_util::StreamExt;
use tokio::task::JoinHandle;
use uq_core::client::DesktopProxy;

use crate::pages::wallpaper::{self, Op, Outcome, WallpaperState};
use crate::session;

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
        /// `{outputs: {"<name>"|"*": entry}, problems: {"<name>": "<message>"}}`; `{}` until uqd
        /// has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused change, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight.
        #[qproperty(bool, busy)]
        type WallpaperModel = super::WallpaperModelRust;

        /// Read everything from uqd again.
        #[qinvokable]
        fn refresh(self: Pin<&mut WallpaperModel>);

        /// Replace one output's entry (`"*"` = all displays) with `json_entry`.
        #[qinvokable]
        fn set_entry(self: Pin<&mut WallpaperModel>, output: &QString, json_entry: &QString);

        /// Change some fields of one output's entry: `json_patch` is merged over the entry uqd
        /// has (or over the `"*"` entry for a display without its own).
        #[qinvokable]
        fn patch_entry(self: Pin<&mut WallpaperModel>, output: &QString, json_patch: &QString);

        /// Remove one output's entry.
        #[qinvokable]
        fn clear_entry(self: Pin<&mut WallpaperModel>, output: &QString);

        /// The local path of a dialog's `file://` URL, percent-decoded; "" if it isn't local.
        #[qinvokable]
        fn local_path(self: &WallpaperModel, url: &QString) -> QString;
    }

    impl cxx_qt::Threading for WallpaperModel {}
    impl cxx_qt::Initialize for WallpaperModel {}
}

use qobject::WallpaperModel;

pub struct WallpaperModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    in_flight: usize,
    /// `None` without a session bus (the offscreen screenshot run): the page reads as "uqd down".
    state: Option<(tokio::runtime::Handle, Arc<WallpaperState>)>,
    watcher: Option<JoinHandle<()>>,
}

impl Default for WallpaperModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            in_flight: 0,
            state: session::bus()
                .map(|(handle, conn)| (handle, Arc::new(WallpaperState::new(conn)))),
            watcher: None,
        }
    }
}

impl Drop for WallpaperModelRust {
    fn drop(&mut self) {
        if let Some(task) = self.watcher.take() {
            task.abort();
        }
    }
}

impl cxx_qt::Initialize for WallpaperModel {
    fn initialize(mut self: Pin<&mut Self>) {
        let Some((handle, state)) = self.state.clone() else {
            return;
        };
        let qt = self.qt_thread();
        let conn = session::bus().map(|(_, c)| c);
        let task = handle.spawn(async move {
            if let Some(conn) = conn {
                follow_uqd(conn, state, qt).await;
            }
        });
        self.as_mut().rust_mut().watcher = Some(task);
        self.refresh();
    }
}

/// Re-read the state whenever uqd announces a change to the `wallpaper` section.
async fn follow_uqd(
    conn: zbus::Connection,
    state: Arc<WallpaperState>,
    qt: CxxQtThread<WallpaperModel>,
) {
    let streams = async {
        let proxy = DesktopProxy::new(&conn).await?;
        let sections = proxy
            .receive_section_changed()
            .await?
            .filter(|s| std::future::ready(s.args().is_ok_and(|a| *a.domain() == "wallpaper")))
            .map(|_| ())
            .boxed();
        zbus::Result::Ok(sections)
    };
    let mut changes = match streams.await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("uq-settings: can't follow uqd's changes: {e}");
            return;
        }
    };
    while changes.next().await.is_some() {
        let outcome = state.run(Op::Refresh).await;
        if qt
            .queue(move |m| m.set_state_json(QString::from(&outcome.state.to_string())))
            .is_err()
        {
            return;
        }
    }
}

impl WallpaperModel {
    /// Run `op` off the Qt thread; the result comes back through [`Self::finish`].
    fn start(mut self: Pin<&mut Self>, op: Op) {
        let Some((handle, state)) = self.state.clone() else {
            return;
        };
        let qt = self.qt_thread();
        let count = self.in_flight + 1;
        self.as_mut().rust_mut().in_flight = count;
        self.as_mut().set_busy(true);
        if !matches!(op, Op::Refresh) {
            // A new user action replaces the old message; a refresh keeps it on show.
            self.as_mut().set_error(QString::default());
        }
        handle.spawn(async move {
            let outcome = state.run(op).await;
            let _ = qt.queue(move |m| m.finish(outcome));
        });
    }

    /// A request finished: show its error (or clear the last one) and the state uqd now has.
    fn finish(mut self: Pin<&mut Self>, outcome: Outcome) {
        let count = self.in_flight.saturating_sub(1);
        self.as_mut().rust_mut().in_flight = count;
        self.as_mut().set_busy(count > 0);
        if let Some(error) = &outcome.error {
            self.as_mut().set_error(QString::from(error));
        }
        self.set_state_json(QString::from(&outcome.state.to_string()));
    }

    fn refresh(self: Pin<&mut Self>) {
        self.start(Op::Refresh);
    }

    fn set_entry(self: Pin<&mut Self>, output: &QString, json_entry: &QString) {
        match serde_json::from_str(&json_entry.to_string()) {
            Ok(entry) => self.start(Op::SetEntry {
                output: output.to_string(),
                entry,
            }),
            Err(e) => self.set_error(QString::from(&format!("{output}: not valid JSON ({e})"))),
        }
    }

    fn patch_entry(self: Pin<&mut Self>, output: &QString, json_patch: &QString) {
        match serde_json::from_str(&json_patch.to_string()) {
            Ok(patch) => self.start(Op::PatchEntry {
                output: output.to_string(),
                patch,
            }),
            Err(e) => self.set_error(QString::from(&format!("{output}: not valid JSON ({e})"))),
        }
    }

    fn clear_entry(self: Pin<&mut Self>, output: &QString) {
        self.start(Op::ClearEntry {
            output: output.to_string(),
        });
    }

    fn local_path(&self, url: &QString) -> QString {
        QString::from(
            wallpaper::local_path(&url.to_string())
                .unwrap_or_default()
                .as_str(),
        )
    }
}
