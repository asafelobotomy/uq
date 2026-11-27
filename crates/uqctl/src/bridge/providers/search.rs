//! Searching: the built-in sources (files, apps, contents, settings) and the providers, newest
//! query first.

use super::*;

impl Providers {
    /// Run the file and/or Settings search again for the newest generation (an index just
    /// became ready).
    pub(crate) async fn answer_latest(self: &Arc<Self>, files: bool, settings: bool) {
        let (generation, query) = {
            let st = lock(&self.state);
            (st.generation, st.query.clone())
        };
        if query.is_empty() {
            return;
        }
        if files {
            self.search_files(generation, &query).await;
        }
        if settings {
            self.search_settings(generation, &query).await;
        }
    }

    /// The file groups for `query`, each its own `SearchResults` event, while `generation` is
    /// still the newest. The shown paths are remembered for `Activate`.
    pub(crate) async fn search_files(self: &Arc<Self>, generation: u64, query: &str) {
        let idx = lock(&self.files).index.clone();
        let q = query.to_string();
        let Ok(groups) = tokio::task::spawn_blocking(move || files::search(&idx, &q)).await else {
            return;
        };
        for (kind, items) in groups {
            if !self.remember_files(generation, &items) {
                return;
            }
            let ev = results_event(generation, kind.provider(), kind.name(), "builtin", &items);
            let _ = self.tx.send(ev).await;
        }
    }

    /// Remember `items` as openable for `generation`; false when it is no longer the newest.
    pub(crate) fn remember_files(&self, generation: u64, items: &[ProviderItem]) -> bool {
        let mut st = lock(&self.state);
        if st.generation != generation {
            return false;
        }
        st.files.extend(items.iter().map(|i| i.id.clone()));
        true
    }

    /// Files elsewhere on the system (plocate), when it is installed.
    pub(crate) async fn search_system(self: &Arc<Self>, generation: u64, query: &str) {
        let roots = lock(&self.files).roots.clone();
        let items = files::system(query, &roots).await;
        if items.is_empty() || !self.remember_files(generation, &items) {
            return;
        }
        let k = files::Kind::System;
        let _ = self
            .tx
            .send(results_event(
                generation,
                k.provider(),
                k.name(),
                "builtin",
                &items,
            ))
            .await;
    }

    /// Files whose text holds the query (contents.rs): Baloo's, or a short scan of the index.
    pub(crate) async fn search_contents(self: &Arc<Self>, generation: u64, query: &str) {
        let idx = lock(&self.files).index.clone();
        let me = self.clone();
        let items = contents::search(query, idx, move || me.latest() != generation).await;
        if items.is_empty() || !self.remember_files(generation, &items) {
            return;
        }
        let ev = results_event(
            generation,
            contents::PROVIDER,
            contents::NAME,
            "builtin",
            &items,
        );
        let _ = self.tx.send(ev).await;
    }

    pub(crate) async fn search_settings(self: &Arc<Self>, generation: u64, query: &str) {
        let items = settings_search::search(&lock(&self.settings).rows, query);
        if items.is_empty() || self.latest() != generation {
            return;
        }
        let ev = results_event(generation, "settings", "Settings", "builtin", &items);
        let _ = self.tx.send(ev).await;
    }

    pub(crate) fn latest(&self) -> u64 {
        lock(&self.state).generation
    }

    /// The bridge's `Search` call. Built-ins are emitted before this returns; external providers
    /// answer later, from tasks that stop with the bridge.
    pub(crate) async fn search(
        self: &Arc<Self>,
        link: &Link,
        runs: &Mutex<RunState>,
        args: &[Value],
    ) -> Result<Value, String> {
        let [Value::String(query), generation] = args else {
            return Err("Search takes a query string and a generation".into());
        };
        let generation = generation.as_u64().ok_or("generation must be a u64")?;
        let query = query.trim();
        let prefs = provider_prefs(link);
        let mut builtin: Vec<(&str, &str, ProviderItem)> = Vec::new();
        {
            let mut st = lock(&self.state);
            if generation < st.generation {
                return Ok(Value::Null);
            }
            st.generation = generation;
            st.query = query.to_string();
            st.calc = None;
            st.web = None;
            st.files.clear();
            if provider_enabled(&prefs, "calc")
                && !query.starts_with('>')
                && calc::applies(query)
                && let Some(it) = calc::item(query)
            {
                st.calc = Some(query.to_string());
                builtin.push(("calc", "Calculator", it));
            }
            // Always called, so a stale command is cleared by any later query. With Run off it
            // sees an empty query: nothing is stored, so nothing can be activated either.
            let run_query = if provider_enabled(&prefs, "run") {
                query
            } else {
                ""
            };
            if let Some(it) = run::run_item(run_query, generation, &mut lock(runs)) {
                builtin.push(("run", "Run", it));
            }
            if provider_enabled(&prefs, "web")
                && !query.starts_with(['>', '=', ':'])
                && let Some(a) = web::parse(query)
            {
                st.web = Some(a.url.clone());
                builtin.push(("web", "Web", web::item(query, &a)));
            }
        }
        for (id, name, it) in builtin {
            let ev = results_event(generation, id, name, "builtin", &[it]);
            let _ = self.tx.send(ev).await;
        }

        let defs = lock(&self.defs).current();
        if builtin_sees(query, &defs) {
            if provider_enabled(&prefs, "files") {
                // The first search builds the index when look=see>do_'s open didn't.
                self.refresh_files();
                let (me, q) = (self.clone(), query.to_string());
                tokio::spawn(async move {
                    tokio::select! {
                        _ = me.stop.cancelled() => {}
                        _ = async {
                            me.search_files(generation, &q).await;
                            tokio::join!(
                                me.search_system(generation, &q),
                                me.search_contents(generation, &q)
                            );
                        } => {}
                    }
                });
            }
            if provider_enabled(&prefs, "settings") {
                self.refresh_settings();
                self.search_settings(generation, query).await;
            }
        }
        let routed: Vec<(ProviderDef, String)> = defs
            .iter()
            .filter(|d| provider_enabled(&prefs, &d.bus_name))
            .filter_map(|d| route(query, d, &defs).map(|q| (d.clone(), q)))
            .collect();
        if routed.is_empty() {
            return Ok(Value::Null);
        }
        let Some(conn) = link.connection().await else {
            return Ok(Value::Null);
        };
        for (def, q) in routed {
            let (me, conn) = (self.clone(), conn.clone());
            tokio::spawn(async move {
                tokio::select! {
                    _ = me.stop.cancelled() => {}
                    _ = me.ask(&conn, &def, &q, generation) => {}
                }
            });
        }
        Ok(Value::Null)
    }

    pub(crate) async fn ask(
        &self,
        conn: &zbus::Connection,
        def: &ProviderDef,
        q: &str,
        generation: u64,
    ) {
        if !owned(conn, &def.bus_name).await {
            return;
        }
        let call = async {
            let p = proxy(conn, &def.bus_name).await?;
            p.call_with_flags::<_, _, Vec<RawItem>>("Search", MethodFlags::NO_AUTO_START, &(q,))
                .await
        };
        let raw = match tokio::time::timeout(SEARCH_TIMEOUT, call).await {
            Ok(Ok(Some(r))) => r,
            Ok(Ok(None)) => return,
            Ok(Err(e)) => return self.log_once(&def.bus_name, &e.to_string()),
            Err(_) => return self.log_once(&def.bus_name, "no answer within 300 ms"),
        };
        if self.latest() != generation {
            return;
        }
        let items = clamp_all(raw);
        let dir = def.source_dir.to_string_lossy();
        let ev = results_event(generation, &def.bus_name, &def.name, &dir, &items);
        let _ = self.tx.send(ev).await;
    }
}
