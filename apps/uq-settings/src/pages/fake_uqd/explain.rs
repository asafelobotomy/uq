//! The fake uqd's `Explain`: each key's value and where it comes from (default, theme, profile,
//! user.lua or a rejected write).

use super::*;

impl FakeUqd {
    pub(super) fn explain_now(&self, domain: &str) -> Result<String, FakeError> {
        if self.explain.fails.load(Ordering::SeqCst) {
            return Err(FakeError::Io("can't read the live session".into()));
        }
        let section = self
            .state
            .lock()
            .unwrap()
            .section(domain)
            .map_err(invalid)?;
        let overrides = self.explain.overrides.lock().unwrap();
        if matches!(domain, "env" | "autostart") {
            let rejected = self.explain.startup_rejected.lock().unwrap().clone();
            let rejected = rejected.filter(|(d, _)| d == domain).map(|(_, m)| m);
            let state = self.state.lock().unwrap();
            let files = self.files.lock().unwrap();
            let active = self.profiles.active.lock().unwrap().clone();
            let out = autostart::explain(
                domain,
                &state,
                &files,
                rejected.as_deref(),
                active.as_ref(),
                &overrides,
            );
            return Ok(out.to_string());
        }
        if domain == "keybinds" {
            let kb = self.state.lock().unwrap().keybinds.clone();
            let rejected = self.explain.keybinds_rejected.lock().unwrap().clone();
            let mut out = self.binds.explain(&kb, rejected.as_deref(), &overrides);
            if let Some(a) = &*self.profiles.active.lock().unwrap() {
                uq_core::profile::mark_binds(&mut out, &kb, a);
            }
            return Ok(out.to_string());
        }
        if domain == "rules" {
            // uqd's pure function; overrides are keyed `rules.<row key>`.
            let rules = self.state.lock().unwrap().rules.clone();
            let rejected = self.explain.rules_rejected.lock().unwrap().clone();
            let mut out = uq_core::rules::explain(&rules, None, rejected.as_deref(), "user.lua");
            if let Some(a) = &*self.profiles.active.lock().unwrap() {
                uq_core::profile::mark_rules(&mut out, &rules, a);
            }
            for (key, row) in out.as_object_mut().into_iter().flatten() {
                if let Some(Value::Object(o)) = overrides.get(&format!("rules.{key}")) {
                    for (k, v) in o {
                        row[k] = v.clone();
                    }
                }
            }
            return Ok(out.to_string());
        }
        let mut out = Map::new();
        for row in provenance::options_for(domain) {
            let stored = row.stored(&section);
            let source = if stored.is_null() {
                row.null_source.as_str()
            } else {
                "uq"
            };
            // Like uqd: a value the active profile set and that still stands is the profile's.
            let path: Vec<String> = row.json_key().split('.').map(str::to_string).collect();
            let from_profile = match &*self.profiles.active.lock().unwrap() {
                Some(a) if source == "uq" && holds(a, domain, &path, &stored) => {
                    Some(a.name.clone())
                }
                _ => None,
            };
            let entry = match from_profile {
                Some(name) => {
                    json!({"source": "profile", "live": null, "stored": stored, "detail": {"name": name}})
                }
                None => json!({"source": source, "live": null, "stored": stored, "detail": null}),
            };
            out.insert(row.json_key().to_string(), entry);
        }
        if domain == "input" {
            // Like uqd: whether the section holds a list of gestures; script a `user.lua`
            // gesture with `Explain::set("input.gestures", {source, detail})`.
            let stored = section.get("gestures").cloned().unwrap_or_default();
            let source = if stored.is_null() { "default" } else { "uq" };
            out.insert(
                "gestures".into(),
                json!({"source": source, "live": null, "stored": stored, "detail": null}),
            );
        }
        let mut out = Value::Object(out);
        if domain == "compat" {
            let rejected = self.explain.startup_rejected.lock().unwrap().clone();
            let rejected = rejected.filter(|(d, _)| d == domain).map(|(_, m)| m);
            let state = self.state.lock().unwrap();
            let files = self.files.lock().unwrap();
            compat::explain(&mut out, &state, &files, rejected.as_deref());
            if let Some(a) = &*self.profiles.active.lock().unwrap() {
                uq_core::profile::mark_startup(&mut out, domain, &state, a);
            }
        }
        for (key, entry) in out.as_object_mut().into_iter().flatten() {
            if let Some(Value::Object(o)) = overrides.get(&format!("{domain}.{key}")) {
                for (k, v) in o {
                    entry[k] = v.clone();
                }
            }
        }
        Ok(out.to_string())
    }
}
