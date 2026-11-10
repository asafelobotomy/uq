//! The Appearance page's QML object. It runs [`AppearanceState`] operations on the tokio runtime
//! (never on the Qt thread) and queues each result back.
//!
//! This file sits in `src/` because cxx-qt-build supports one directory of bridge files per QML
//! module (QTBUG-93443), and `app.rs` and `window.rs` are there.

use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;
use futures_util::StreamExt;
use tokio::task::JoinHandle;
use uq_core::client::DesktopProxy;

use crate::pages::appearance::{AppearanceState, Op, Outcome};
use crate::{session, system_lists};

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
        /// `{themes, theme, variant, schedule, appearance, cursorThemes, iconThemes}`; `{}` until
        /// uqd has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused change, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight.
        #[qproperty(bool, busy)]
        type AppearanceModel = super::AppearanceModelRust;

        /// Read everything from uqd again.
        #[qinvokable]
        fn refresh(self: Pin<&mut AppearanceModel>);

        #[qinvokable]
        fn set_theme(self: Pin<&mut AppearanceModel>, id: &QString, variant: &QString);

        /// `json`: `{"mode":"off"}`, `{"mode":"fixed",...}` or `{"mode":"sun",...}`.
        #[qinvokable]
        fn set_schedule(self: Pin<&mut AppearanceModel>, json: &QString);

        /// Set one field of the `appearance` section to `json_value` (`null` = use the theme).
        #[qinvokable]
        fn set_appearance(self: Pin<&mut AppearanceModel>, field: &QString, json_value: &QString);
    }

    impl cxx_qt::Threading for AppearanceModel {}
    impl cxx_qt::Initialize for AppearanceModel {}
}

use qobject::AppearanceModel;

pub struct AppearanceModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    in_flight: usize,
    /// `None` without a session bus (the offscreen screenshot run): the page reads as "uqd down".
    state: Option<(tokio::runtime::Handle, Arc<AppearanceState>)>,
    watcher: Option<JoinHandle<()>>,
}

impl Default for AppearanceModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            in_flight: 0,
            state: session::bus().map(|(handle, conn)| {
                (
                    handle,
                    Arc::new(AppearanceState::new(conn, system_lists::real_dirs())),
                )
            }),
            watcher: None,
        }
    }
}

impl Drop for AppearanceModelRust {
    fn drop(&mut self) {
        if let Some(task) = self.watcher.take() {
            task.abort();
        }
    }
}

impl cxx_qt::Initialize for AppearanceModel {
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

/// Re-read the state whenever uqd announces a change.
async fn follow_uqd(
    conn: zbus::Connection,
    state: Arc<AppearanceState>,
    qt: CxxQtThread<AppearanceModel>,
) {
    let streams = async {
        let proxy = DesktopProxy::new(&conn).await?;
        let sections = proxy.receive_section_changed().await?.map(|_| ()).boxed();
        let themes = proxy.receive_theme_changed().await?.map(|_| ()).boxed();
        let settings = proxy.receive_settings_changed().await?.map(|_| ()).boxed();
        zbus::Result::Ok(futures_util::stream::select_all([
            sections, themes, settings,
        ]))
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

impl AppearanceModel {
    /// Run `op` off the Qt thread; the result comes back through [`Self::finish`].
    fn start(mut self: Pin<&mut Self>, op: Op) {
        let Some((handle, state)) = self.state.clone() else {
            return;
        };
        let qt = self.qt_thread();
        let count = self.in_flight + 1;
        self.as_mut().rust_mut().in_flight = count;
        self.as_mut().set_busy(true);
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
        self.as_mut()
            .set_error(QString::from(outcome.error.as_deref().unwrap_or("")));
        self.set_state_json(QString::from(&outcome.state.to_string()));
    }

    fn refresh(self: Pin<&mut Self>) {
        self.start(Op::Refresh);
    }

    fn set_theme(self: Pin<&mut Self>, id: &QString, variant: &QString) {
        self.start(Op::SetTheme {
            id: id.to_string(),
            variant: variant.to_string(),
        });
    }

    fn set_schedule(self: Pin<&mut Self>, json: &QString) {
        self.start(Op::SetSchedule(json.to_string()));
    }

    fn set_appearance(self: Pin<&mut Self>, field: &QString, json_value: &QString) {
        match serde_json::from_str(&json_value.to_string()) {
            Ok(value) => self.start(Op::SetField {
                field: field.to_string(),
                value,
            }),
            Err(e) => self.set_error(QString::from(&format!("{field}: not valid JSON ({e})"))),
        }
    }
}
