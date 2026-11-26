//! A page's declared commands: running, streaming, confirming and watching them, one slot each.

use super::*;

impl PluginHost {
    pub(crate) fn run_command(
        self: Pin<&mut Self>,
        page: &QString,
        command: &QString,
        args_json: &QString,
    ) -> i32 {
        self.begin(page, command, args_json, false)
    }

    pub(crate) fn stream_command(
        self: Pin<&mut Self>,
        page: &QString,
        command: &QString,
        args_json: &QString,
    ) -> i32 {
        self.begin(page, command, args_json, true)
    }

    pub(crate) fn begin(
        mut self: Pin<&mut Self>,
        page: &QString,
        command: &QString,
        args_json: &QString,
        stream: bool,
    ) -> i32 {
        let id = self.as_mut().next_id();
        let page = page.to_string();
        let manifest = plugins::manifest(&page);
        let (spec, args) = match prepare(
            manifest.clone(),
            &command.to_string(),
            &args_json.to_string(),
        ) {
            Ok(x) => x,
            Err(e) => {
                self.deliver(id, error(e));
                return id;
            }
        };
        if stream && spec.output != Output::Ndjson {
            self.deliver(
                id,
                error(format!(
                    "{} doesn't stream (it needs output = \"ndjson\")",
                    spec.id
                )),
            );
            return id;
        }
        let Some(text) = spec.confirm_text(&args) else {
            self.start(id, page, spec, args, stream);
            return id;
        };
        let Some(preview) = spec.preview.clone() else {
            self.ask(
                id,
                Pending {
                    page,
                    spec,
                    args,
                    stream,
                    refused: false,
                },
                &text,
                "",
            );
            return id;
        };
        // Work out what would change first; the person is asked only when that worked.
        let pspec = manifest
            .as_ref()
            .and_then(|m| command::find(&m.commands, &preview).ok().cloned());
        let (Some(pspec), Some(rt)) = (pspec, RUNTIME.get()) else {
            self.deliver(id, error("can't work out what would change right now"));
            return id;
        };
        let qt = self.qt_thread();
        rt.spawn(async move {
            let plan = match runner().run(&pspec, &args).await {
                Ok(r) if r.code == 0 => plan::normalize(&r.output),
                Ok(r) => Err(r.to_json()["error"]
                    .as_str()
                    .unwrap_or("it failed")
                    .to_string()),
                Err(e) => Err(e.to_string()),
            };
            let _ = qt.queue(move |mut host| match plan {
                Ok(plan) => {
                    let refused = plan["would_refuse"].as_bool().unwrap_or(false);
                    let json = plan.to_string();
                    host.as_mut().ask(
                        id,
                        Pending {
                            page,
                            spec,
                            args,
                            stream,
                            refused,
                        },
                        &text,
                        &json,
                    );
                }
                Err(e) => {
                    eprintln!("uq-settings: page {page}: preview of {}: {e}", spec.id);
                    host.deliver(
                        id,
                        error(format!(
                            "Couldn't work out what would change, so nothing was done: {e}"
                        )),
                    );
                }
            });
        });
        id
    }

    /// Hold `pending` and show the question (and the plan, "" for none).
    pub(super) fn ask(
        mut self: Pin<&mut Self>,
        id: i32,
        pending: Pending,
        text: &str,
        plan_json: &str,
    ) {
        let page = QString::from(&pending.page);
        self.as_mut().rust_mut().pending.insert(id, pending);
        self.confirm_requested(id, page, QString::from(text), QString::from(plan_json));
    }

    pub(crate) fn answer(mut self: Pin<&mut Self>, request: i32, confirmed: bool) {
        let Some(p) = self.as_mut().rust_mut().pending.remove(&request) else {
            return;
        };
        if confirmed && !p.refused {
            self.start(request, p.page, p.spec, p.args, p.stream);
        } else {
            self.deliver(request, json!({"error": "Cancelled", "cancelled": true}));
        }
    }

    pub(crate) fn start(
        &self,
        id: i32,
        page: String,
        spec: CommandSpec,
        args: Vec<String>,
        stream: bool,
    ) {
        let token = match busy().try_acquire(&page, &spec.id) {
            Ok(t) => t,
            Err(e) => return self.deliver(id, command::error_json(&e)),
        };
        let Some(rt) = RUNTIME.get() else {
            return self.deliver(id, error("commands can't run right now"));
        };
        let qt = self.qt_thread();
        let events = self.qt_thread();
        rt.spawn(async move {
            let result = if stream {
                let mut on_event = move |v: Value| {
                    let json = QString::from(&v.to_string());
                    let _ = events.queue(move |host| host.progress(id, json));
                };
                runner().stream(&spec, &args, &mut on_event).await
            } else {
                runner().run(&spec, &args).await
            };
            drop(token);
            let value = match &result {
                Ok(r) => {
                    if r.code != 0 {
                        eprintln!(
                            "uq-settings: page {page}: command {} exited with {}: {}",
                            spec.id,
                            r.code,
                            r.stderr_head.trim()
                        );
                    }
                    r.to_json()
                }
                Err(e) => {
                    if !matches!(e, CommandError::Busy) {
                        eprintln!("uq-settings: page {page}: command {}: {e}", spec.id);
                    }
                    command::error_json(e)
                }
            };
            let json = QString::from(&value.to_string());
            let _ = qt.queue(move |host| host.finished(id, json));
        });
    }

    pub(crate) fn watch_command(
        mut self: Pin<&mut Self>,
        page: &QString,
        command: &QString,
    ) -> i32 {
        let (page, command) = (page.to_string(), command.to_string());
        let spec = match watch_spec(plugins::manifest(&page), &command) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("uq-settings: page {page}: can't watch {command}: {e}");
                return 0;
            }
        };
        let watches = &mut self.as_mut().rust_mut().get_mut().watches;
        watches.retain(|_, (_, task)| !task.is_finished());
        let open = watches.values().filter(|(p, _)| *p == page).count();
        if open >= MAX_WATCHES {
            eprintln!("uq-settings: page {page}: more than {MAX_WATCHES} watches at once");
            return 0;
        }
        let Some(rt) = RUNTIME.get() else {
            return 0;
        };
        let id = self.as_mut().next_id();
        let qt = self.qt_thread();
        let label = format!("{page}: {command}");
        let task = rt.spawn(async move {
            let uses = |b: Bus| spec.signals.iter().any(|s| s.bus == b);
            let session = if uses(Bus::Session) {
                connection(Bus::Session).await.ok()
            } else {
                None
            };
            let system = if uses(Bus::System) {
                connection(Bus::System).await.ok()
            } else {
                None
            };
            let home = std::env::var_os("HOME").map_or_else(|| "/".into(), PathBuf::from);
            let watch = command_watch::start(
                &spec,
                &home,
                session.as_ref(),
                system.as_ref(),
                WATCH_DEBOUNCE,
            )
            .await;
            let mut watch = match watch {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("uq-settings: page {label}: can't follow changes: {e}");
                    return;
                }
            };
            while watch.changes.recv().await.is_some() {
                if qt.queue(move |host| host.command_changed(id)).is_err() {
                    return;
                }
            }
        });
        self.as_mut().rust_mut().watches.insert(id, (page, task));
        id
    }

    pub(crate) fn unwatch(mut self: Pin<&mut Self>, watch: i32) {
        if let Some((_, task)) = self.as_mut().rust_mut().watches.remove(&watch) {
            task.abort();
        }
    }
}
