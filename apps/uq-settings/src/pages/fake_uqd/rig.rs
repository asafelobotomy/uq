//! The rig tests use: the fake uqd on a private bus, with a client.

use super::*;

pub struct Rig {
    /// Held so the bus lives as long as the rig.
    #[allow(dead_code, reason = "held, not read, by most tests")]
    pub bus: Bus,
    /// The fake's own connection; it owns `org.uq.Desktop1` (and can emit its signals).
    #[allow(dead_code, reason = "held, not read, by most tests")]
    pub uqd: zbus::Connection,
    /// A connection for the code under test.
    pub client: zbus::Connection,
    pub state: Arc<Mutex<State>>,
    pub verify_error: Arc<Mutex<Option<String>>>,
    /// The text the next `SetSection` is refused with.
    pub refuse_next: Arc<Mutex<Option<String>>>,
    /// What `Explain` answers, and what `Reapply` was asked.
    pub explain: Explain,
    /// The journal and the restore points.
    pub history: History,
    /// The live outputs and the pending display change.
    pub displays: Displays,
    /// The stored profiles and the active one.
    pub profiles: FakeProfiles,
    /// The open windows and the scripted picks.
    pub windows: FakeWindows,
    /// The found startup entries and the `ShellReady` count.
    pub autostart: FakeAutostart,
    /// The system and user themes, and the theme calls made.
    pub themes: FakeThemes,
    /// The shortcuts' `user.lua` and the key-capture pause.
    pub binds: FakeBinds,
    /// What the file sync found, for `Explain`.
    pub files: Arc<Mutex<FileReport>>,
}

/// uqd's revert timer firing: the pending display change is dropped and `MonitorsChanged` sent.
pub async fn expire(rig: &Rig) {
    rig.displays.pending.lock().unwrap().take();
    let iface = rig
        .uqd
        .object_server()
        .interface::<_, FakeUqd>("/org/uq/Desktop1")
        .await
        .unwrap();
    FakeUqd::monitors_changed(iface.signal_emitter())
        .await
        .unwrap();
}

/// A private bus with a [`FakeUqd`] serving `state`.
pub async fn rig(state: State) -> Rig {
    let bus = private_bus();
    let state = Arc::new(Mutex::new(state));
    let verify_error = Arc::new(Mutex::new(None));
    let refuse_next = Arc::new(Mutex::new(None));
    let explain = Explain::default();
    let history = History::default();
    let displays = Displays::default();
    let profiles = FakeProfiles::default();
    let windows = FakeWindows::default();
    let autostart = FakeAutostart::default();
    let themes = FakeThemes::default();
    let binds = FakeBinds::default();
    let files = Arc::new(Mutex::new(FileReport::default()));
    let uqd = connect(&bus).await;
    uqd.object_server()
        .at(
            "/org/uq/Desktop1",
            FakeUqd {
                state: state.clone(),
                verify_error: verify_error.clone(),
                refuse_next: refuse_next.clone(),
                explain: explain.clone(),
                history: history.clone(),
                displays: displays.clone(),
                profiles: profiles.clone(),
                windows: windows.clone(),
                autostart: autostart.clone(),
                themes: themes.clone(),
                binds: binds.clone(),
                files: files.clone(),
            },
        )
        .await
        .unwrap();
    uqd.request_name(DESKTOP_NAME).await.unwrap();
    let client = connect(&bus).await;
    Rig {
        bus,
        uqd,
        client,
        state,
        verify_error,
        refuse_next,
        explain,
        history,
        displays,
        profiles,
        windows,
        autostart,
        themes,
        binds,
        files,
    }
}
