//! The bridge's session: reading the shell's requests, writing replies and events, relaying
//! lock requests.

use super::*;

/// Run the notification-owner watch on the bridge's bus connection, waiting for the bus to be
/// reachable and starting over if the connection goes away.
pub(crate) async fn watch_notif_owner(
    link: Arc<uqd::Link>,
    tx: mpsc::Sender<Outbound>,
    stop: CancellationToken,
) {
    while !stop.is_cancelled() {
        if let Some(conn) = link.connection().await {
            notifowner::watch(conn, tx.clone(), stop.clone()).await;
        }
        tokio::select! { _ = stop.cancelled() => return, _ = tokio::time::sleep(Duration::from_secs(5)) => {} }
    }
}

/// Owns stdout: writes each message as one line. A reply too large for one line is replaced by
/// an error for the same id; an over-long event can only be dropped.
pub(crate) async fn write_loop<W: AsyncWrite + Unpin>(mut w: W, mut rx: mpsc::Receiver<Outbound>) {
    while let Some(msg) = rx.recv().await {
        let mut line = encode(&msg);
        if line.len() > MAX_LINE + 1 {
            match &msg {
                Outbound::Ok { id, .. } | Outbound::Err { id, .. } => {
                    line = encode(&Outbound::Err {
                        id: *id,
                        err: "response too large".into(),
                    });
                }
                Outbound::Event { event, .. } => {
                    eprintln!("uqctl bridge: dropping over-long {event} event");
                    continue;
                }
            }
        }
        if w.write_all(&line).await.is_err() || w.flush().await.is_err() {
            eprintln!("uqctl bridge: stdout closed");
            break;
        }
    }
    // Everything handed over is written (ThreadStdout writes on its own thread).
    let _ = w.shutdown().await;
}

/// Relay lock-notify states: each change (and the first report) becomes a `LockState` event, then
/// goes to `hints` for logind's `SetLockedHint`. Handing the hint to `Logind::hint_loop` through a
/// latest-value channel means a logind that stops answering never delays a `LockState`. `known`
/// is a state already reported before Hello.
pub(crate) async fn relay_locks(
    mut locks: mpsc::UnboundedReceiver<bool>,
    mut known: Option<bool>,
    hints: watch::Sender<Option<bool>>,
    tx: mpsc::Sender<Outbound>,
) {
    let mut sent = None;
    loop {
        if let Some(locked) = known.take()
            && sent != Some(locked)
        {
            sent = Some(locked);
            let ev = Outbound::Event {
                event: "LockState".into(),
                data: json!({"locked": locked}),
            };
            if tx.send(ev).await.is_err() {
                return;
            }
            hints.send_replace(Some(locked));
        }
        match locks.recv().await {
            Some(locked) => known = Some(locked),
            None => return,
        }
    }
}

/// As `run`, against the session bus at `addr` and the system bus at `system` (None: the real
/// ones), with lock states from `locks`. Tests pass private buses and their own lock source, so
/// they never reach a Wayland display. `roots` None runs without widgets and app actions.
pub async fn run_with_bus<R, W>(
    stdin: R,
    stdout: W,
    addr: Option<&str>,
    system: Option<&str>,
    mut locks: mpsc::UnboundedReceiver<bool>,
    roots: Option<IntegrationRoots>,
) -> ExitCode
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (out_tx, out_rx) = mpsc::channel::<Outbound>(256);
    let writer = tokio::spawn(write_loop(stdout, out_rx));
    let (in_tx, mut in_rx) = mpsc::channel(64);
    let reader = tokio::spawn(read_lines(stdin, in_tx));

    // Learn uqd's state first so Hello (always the first line out) is truthful.
    let link = Arc::new(uqd::Link::open(addr).await);
    // The latest lock state lock-notify has reported by now, if any.
    let mut known_lock = None;
    while let Ok(locked) = locks.try_recv() {
        known_lock = Some(locked);
    }
    // Whether Welcome is due (null when uqd isn't up yet or doesn't answer within READY_WAIT)
    // and held back by bindle (§5.1). A null with uqd_up true makes the shell ask again
    // (WelcomeStatus) after its grace period; a null with uqd down waits for UqdUp.
    let welcome = welcome::status(
        &link,
        welcome::runtime_dir().as_deref(),
        Some(uqd::READY_WAIT),
    )
    .await;
    let hello = Outbound::Event {
        event: "Hello".into(),
        data: json!({"v": 1, "uqd_up": link.is_up(), "locked": known_lock.unwrap_or(false),
            "welcome_pending": welcome["pending"], "welcome_deferred": welcome["deferred"]}),
    };
    let _ = out_tx.send(hello).await;

    // Background tasks hold sender clones, so shutdown is by cancellation, not by sender drop.
    let stop = CancellationToken::new();
    let watcher = tokio::spawn({
        let (link, tx, stop) = (link.clone(), out_tx.clone(), stop.clone());
        async move { link.watch(tx, stop).await }
    });

    let net = net::Net::new(
        net::NetClient::new(),
        out_tx.clone(),
        link.settings_rx(),
        stop.clone(),
        net::SETTLE,
    );
    let weather = tokio::spawn(net.clone().weather_loop(net::WEATHER_EVERY));
    let privacy = tokio::spawn(net.clone().privacy_loop());
    // The data islands: each fetches or samples only while its island is placed.
    let cache = feeds::cache_dir();
    let stocks = feeds::FeedLoop::new(
        stocks::Stocks::new(),
        out_tx.clone(),
        link.settings_rx(),
        stop.clone(),
        stocks::EVERY,
        feeds::RETRY.to_vec(),
        cache.clone(),
        feeds::MANUAL_GAP,
    );
    let news = feeds::FeedLoop::new(
        news::News::new(),
        out_tx.clone(),
        link.settings_rx(),
        stop.clone(),
        news::EVERY,
        feeds::RETRY.to_vec(),
        cache,
        feeds::MANUAL_GAP,
    );
    let stocks_task = tokio::spawn(stocks.clone().run());
    let news_task = tokio::spawn(news.clone().run());
    let sysmon_task = tokio::spawn(sysmon::SysMon::new().run(
        out_tx.clone(),
        link.settings_rx(),
        stop.clone(),
        sysmon::EVERY,
    ));
    let worldclock_task = tokio::spawn(worldclock::run(
        out_tx.clone(),
        link.settings_rx(),
        stop.clone(),
    ));
    let notif = tokio::spawn(watch_notif_owner(
        link.clone(),
        out_tx.clone(),
        stop.clone(),
    ));
    let logind = logind::Logind::new(out_tx.clone());
    let sleep_watch = tokio::spawn(
        logind
            .clone()
            .connect_and_run(system.map(str::to_owned), stop.clone()),
    );
    let (hint_tx, hint_rx) = watch::channel(None);
    let hint_task = tokio::spawn(logind.clone().hint_loop(hint_rx));
    let (idle_tx, idle_rx) = watch::channel(None);
    let idle_task = tokio::spawn(logind.clone().idle_loop(idle_rx));
    let lock_relay = tokio::spawn(relay_locks(locks, known_lock, hint_tx, out_tx.clone()));

    let runs = Arc::new(Mutex::new(run::RunState::default()));
    let providers = providers::Providers::new(
        out_tx.clone(),
        stop.clone(),
        roots
            .as_ref()
            .map_or_else(uq_core::launcher_providers::default_dirs, |r| {
                r.providers.clone()
            }),
    );
    // Widgets and app actions: their first events follow Hello.
    let (widgets, actions) = match &roots {
        Some(r) => (
            Some(widgets::Widgets::new(
                out_tx.clone(),
                stop.clone(),
                r.widgets.clone(),
                r.bin_dir.clone(),
                addr,
                system,
            )),
            Some(appactions::AppActions::new(
                out_tx.clone(),
                stop.clone(),
                r.actions.clone(),
            )),
        ),
        None => (None, None),
    };
    let widget_task = widgets.clone().map(|w| tokio::spawn(w.run()));
    let action_task = actions.clone().map(|a| tokio::spawn(a.run()));
    let services = Services {
        link: link.clone(),
        net: net.clone(),
        stocks: stocks.clone(),
        news: news.clone(),
        runs: runs.clone(),
        providers: providers.clone(),
        logind: logind.clone(),
        idle: Arc::new(idle_tx),
        procs: Arc::new(procusage::ProcUsage::new()),
        widgets,
        actions,
    };
    let slots = Arc::new(Semaphore::new(MAX_IN_FLIGHT));
    let mut requests = tokio::task::JoinSet::new();
    let mut shell_seen = false;
    while let Some(item) = in_rx.recv().await {
        match item {
            Ok(Inbound::Request { id, call, args }) => {
                if !shell_seen {
                    // `Hello` is ours, so the shell's first request is what shows it is up (R5).
                    shell_seen = true;
                    let link = link.clone();
                    requests.spawn(async move { link.shell_ready().await });
                }
                let Ok(permit) = slots.clone().acquire_owned().await else {
                    break;
                };
                let (services, tx) = (services.clone(), out_tx.clone());
                requests.spawn(async move {
                    let reply = match handle(&services, &call, &args).await {
                        Ok(ok) => Outbound::Ok { id, ok },
                        Err(err) => Outbound::Err { id, err },
                    };
                    let _ = tx.send(reply).await;
                    drop(permit);
                });
            }
            Err(Rejected {
                id: Some(id),
                error,
            }) => {
                let _ = out_tx
                    .send(Outbound::Err {
                        id,
                        err: error.to_string(),
                    })
                    .await;
            }
            Err(Rejected { id: None, error }) => {
                eprintln!("uqctl bridge: dropped input line: {error}");
            }
        }
        // Reap finished request tasks so the set stays small.
        while requests.try_join_next().is_some() {}
    }

    // stdin EOF: stop background work, then let the writer drain what is queued (bounded).
    stop.cancel();
    requests.abort_all();
    // Abort rather than await: the watcher may be blocked on a full channel or the bus.
    watcher.abort();
    weather.abort();
    privacy.abort();
    stocks_task.abort();
    news_task.abort();
    sysmon_task.abort();
    worldclock_task.abort();
    notif.abort();
    sleep_watch.abort();
    // Dropping the receiver ends the lock-notify thread at its next event; it is never joined.
    lock_relay.abort();
    // It may be stuck in a SetLockedHint call that logind never answers.
    hint_task.abort();
    idle_task.abort();
    if let Some(t) = widget_task {
        t.abort();
    }
    if let Some(t) = action_task {
        t.abort();
    }
    drop(services);
    drop(net);
    drop(stocks);
    drop(news);
    drop(providers);
    drop(logind);
    reader.abort();
    drop(out_tx);
    if tokio::time::timeout(DRAIN, writer).await.is_err() {
        eprintln!("uqctl bridge: output did not drain in time");
    }
    ExitCode::SUCCESS
}
