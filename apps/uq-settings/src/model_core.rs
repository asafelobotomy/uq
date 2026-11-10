//! The QObject side every page model shares: it runs [`PageLogic`] operations on the tokio
//! runtime (never on the Qt thread), queues each result back, and re-reads the state when uqd
//! announces a change the page follows. Not a bridge file: each `*_model.rs` keeps its own bridge
//! and implements [`PageModel`].

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::CxxQtThread;
use cxx_qt_lib::QString;
use futures_util::StreamExt;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use uq_core::client::DesktopProxy;

use crate::pages::runner::{self, Outcome, PageLogic};
use crate::session;

/// Which of uqd's change signals make a page read its state again.
#[derive(Debug, Clone, Copy)]
pub struct Follow {
    /// `SectionChanged` for these domains.
    pub sections: &'static [&'static str],
    /// `ThemeChanged`.
    pub theme: bool,
    /// `SettingsChanged`.
    pub settings: bool,
    /// `HistoryChanged`.
    pub history: bool,
    /// `MonitorsChanged`: a display change was applied, kept or put back.
    pub monitors: bool,
    /// `ProfileChanged`: the active profile or the list of profiles changed.
    pub profiles: bool,
}

/// Whether a `SectionChanged(domain, …)` concerns the page.
pub fn wants(f: &Follow, domain: &str) -> bool {
    f.sections.contains(&domain)
}

/// The Rust state a page model keeps beside its properties.
pub struct ModelCore<L: PageLogic> {
    in_flight: usize,
    /// `None` without a session bus (the offscreen screenshot run): the page reads as "uqd down".
    logic: Option<(Handle, Arc<L>)>,
    watcher: Option<JoinHandle<()>>,
}

impl<L: PageLogic> ModelCore<L> {
    pub fn new(make: impl FnOnce(zbus::Connection) -> L) -> Self {
        Self {
            in_flight: 0,
            logic: session::bus().map(|(handle, conn)| (handle, Arc::new(make(conn)))),
            watcher: None,
        }
    }
}

impl<L: PageLogic> Drop for ModelCore<L> {
    fn drop(&mut self) {
        if let Some(task) = self.watcher.take() {
            task.abort();
        }
    }
}

/// A page's QObject: its [`ModelCore`] and the three properties every page has. It is
/// `'static` because its `CxxQtThread` goes into tasks on the runtime.
pub trait PageModel: cxx_qt::Threading + Sized + 'static {
    type Logic: PageLogic;
    const FOLLOW: Follow;

    fn core(&self) -> &ModelCore<Self::Logic>;
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<Self::Logic>;
    fn show_busy(self: Pin<&mut Self>, busy: bool);
    fn show_error(self: Pin<&mut Self>, e: QString);
    fn show_state(self: Pin<&mut Self>, json: QString);
}

/// Start following uqd's changes, then read the state.
pub fn initialize<M: PageModel>(mut m: Pin<&mut M>) {
    let Some((handle, logic)) = m.core().logic.clone() else {
        return;
    };
    let qt = m.qt_thread();
    let conn = session::bus().map(|(_, c)| c);
    let task = handle.spawn(async move {
        if let Some(conn) = conn {
            follow_uqd(conn, logic, qt).await;
        }
    });
    m.as_mut().core_mut().watcher = Some(task);
    start(m, M::Logic::refresh());
}

/// Re-read the state whenever uqd announces a change in [`PageModel::FOLLOW`]. Ends once the
/// QObject is gone.
async fn follow_uqd<M: PageModel>(
    conn: zbus::Connection,
    logic: Arc<M::Logic>,
    qt: CxxQtThread<M>,
) {
    let follow = M::FOLLOW;
    let streams = async {
        let proxy = DesktopProxy::new(&conn).await?;
        let mut all = vec![
            proxy
                .receive_section_changed()
                .await?
                .filter(move |s| {
                    std::future::ready(s.args().is_ok_and(|a| wants(&follow, a.domain())))
                })
                .map(|_| ())
                .boxed(),
        ];
        if follow.theme {
            all.push(proxy.receive_theme_changed().await?.map(|_| ()).boxed());
        }
        if follow.settings {
            all.push(proxy.receive_settings_changed().await?.map(|_| ()).boxed());
        }
        if follow.history {
            all.push(proxy.receive_history_changed().await?.map(|_| ()).boxed());
        }
        if follow.monitors {
            all.push(proxy.receive_monitors_changed().await?.map(|_| ()).boxed());
        }
        if follow.profiles {
            all.push(proxy.receive_profile_changed().await?.map(|_| ()).boxed());
        }
        zbus::Result::Ok(futures_util::stream::select_all(all))
    };
    let mut changes = match streams.await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("uq-settings: can't follow uqd's changes: {e}");
            return;
        }
    };
    while changes.next().await.is_some() {
        let outcome = runner::run(&*logic, M::Logic::refresh()).await;
        // A refresh's message (the Displays page's "restored") shows; none leaves the last one.
        let shown = qt.queue(move |mut m| {
            if let Some(error) = &outcome.error {
                m.as_mut().show_error(QString::from(error));
            }
            m.show_state(QString::from(&outcome.state.to_string()));
        });
        if shown.is_err() {
            return;
        }
    }
}

/// Run `op` off the Qt thread; the result comes back through [`finish`].
pub fn start<M: PageModel>(m: Pin<&mut M>, op: <M::Logic as PageLogic>::Op) {
    // A new user action replaces the old message; a refresh keeps it on show.
    let clears_error = !M::Logic::is_refresh(&op);
    spawn(m, clears_error, move |logic| async move {
        runner::run(&*logic, op).await
    });
}

/// Ask uqd to apply `domain` to the session again (a row's **Re-apply**), then read the state
/// back. Only for a domain the page follows: its rows are the ones that show a source.
pub fn reapply<M: PageModel>(m: Pin<&mut M>, domain: String) {
    if !wants(&M::FOLLOW, &domain) {
        m.show_error(QString::from(&format!("this page doesn't show {domain}")));
        return;
    }
    let Some((_, conn)) = session::bus() else {
        return;
    };
    spawn(m, true, move |logic| async move {
        runner::reapply(&*logic, &conn, &domain).await
    });
}

/// Run `job` on the runtime, marking the page busy until it is done.
fn spawn<M, F, Fut>(mut m: Pin<&mut M>, clears_error: bool, job: F)
where
    M: PageModel,
    F: FnOnce(Arc<M::Logic>) -> Fut + Send + 'static,
    Fut: Future<Output = Outcome> + Send + 'static,
{
    let Some((handle, logic)) = m.core().logic.clone() else {
        return;
    };
    let qt = m.qt_thread();
    m.as_mut().core_mut().in_flight += 1;
    m.as_mut().show_busy(true);
    if clears_error {
        m.as_mut().show_error(QString::default());
    }
    handle.spawn(async move {
        let outcome = job(logic).await;
        let _ = qt.queue(move |m| finish(m, outcome));
    });
}

/// A request finished: show its error (or clear the last one) and the state uqd now has.
fn finish<M: PageModel>(mut m: Pin<&mut M>, outcome: Outcome) {
    let core = m.as_mut().core_mut();
    core.in_flight = core.in_flight.saturating_sub(1);
    let busy = core.in_flight > 0;
    m.as_mut().show_busy(busy);
    if let Some(error) = &outcome.error {
        m.as_mut().show_error(QString::from(error));
    }
    m.show_state(QString::from(&outcome.state.to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wants_filters_by_domain() {
        let wallpaper = Follow {
            sections: &["wallpaper"],
            theme: false,
            settings: false,
            history: false,
            monitors: false,
            profiles: false,
        };
        assert!(wants(&wallpaper, "wallpaper"));
        assert!(!wants(&wallpaper, "appearance"));
        assert!(!wants(&wallpaper, ""));
        let two = Follow {
            sections: &["look", "layouts"],
            theme: true,
            settings: true,
            history: false,
            monitors: false,
            profiles: false,
        };
        assert!(wants(&two, "look") && wants(&two, "layouts"));
        assert!(!wants(&two, "shell"));
    }
}
