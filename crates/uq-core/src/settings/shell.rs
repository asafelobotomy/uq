//! The `shell` section: which bar islands show, the clock format, the launcher's provider
//! order and enablement, and the terminal for `Terminal=true` launches. The shell reads it
//! through `SettingsChanged`; uqd renders nothing for it.
//!
//! Loading is lenient (wrong-typed or unknown values fall back or are dropped, so a bad
//! `[shell]` never costs the user their theme or pins); the API is strict
//! ([`ShellSettings::from_section`]).

use super::{SettingError, check_known, invalid, valid_pinned_id};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use zbus::names::WellKnownName;

/// Providers the launcher always has, in their default order. The apps group is not one.
pub const BUILTIN_PROVIDERS: [&str; 2] = ["calc", "run"];

const MAX_PROVIDERS: usize = 64;

/// Load-time leniency: a value of the wrong type becomes `fallback()`.
fn lenient_or<'de, D, T>(d: D, fallback: fn() -> T) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    Ok(T::deserialize(Value::deserialize(d)?).unwrap_or_else(|_| fallback()))
}

fn true_() -> bool {
    true
}

fn lenient_true<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    lenient_or(d, true_)
}

fn lenient_islands<'de, D: Deserializer<'de>>(d: D) -> Result<Islands, D::Error> {
    lenient_or(d, Islands::default)
}

fn lenient_terminal<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => Some(s),
        _ => None,
    })
}

/// Entries that aren't `{id, enabled}` tables are dropped; a non-array is an empty list.
fn lenient_providers<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<ProviderPref>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Array(a) => a
            .into_iter()
            .filter_map(|v| serde_json::from_value::<ProviderPref>(v).ok())
            .collect(),
        _ => Vec::new(),
    })
}

/// The five islands of `shell/bar/Bar.qml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Islands {
    #[serde(deserialize_with = "lenient_true")]
    pub system: bool,
    #[serde(deserialize_with = "lenient_true")]
    pub work: bool,
    #[serde(deserialize_with = "lenient_true")]
    pub time: bool,
    #[serde(deserialize_with = "lenient_true")]
    pub notif: bool,
    #[serde(deserialize_with = "lenient_true")]
    pub status: bool,
}

impl Default for Islands {
    fn default() -> Self {
        Self {
            system: true,
            work: true,
            time: true,
            notif: true,
            status: true,
        }
    }
}

impl Islands {
    pub const FIELDS: [&str; 5] = ["system", "work", "time", "notif", "status"];
}

/// One launcher provider's preference. `id` is `calc`, `run` or a D-Bus well-known name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPref {
    pub id: String,
    #[serde(default = "true_", deserialize_with = "lenient_true")]
    pub enabled: bool,
}

impl ProviderPref {
    pub const FIELDS: [&str; 2] = ["id", "enabled"];

    pub fn valid_id(id: &str) -> bool {
        BUILTIN_PROVIDERS.contains(&id)
            || (!id.starts_with(':') && WellKnownName::try_from(id).is_ok())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellSettings {
    #[serde(deserialize_with = "lenient_islands")]
    pub islands: Islands,
    /// The user's order; ids the launcher knows but this lacks follow in default order.
    #[serde(deserialize_with = "lenient_providers")]
    pub providers: Vec<ProviderPref>,
    /// `true` is `HH:mm`, `false` is the 12-hour clock.
    #[serde(deserialize_with = "lenient_true")]
    pub clock_24h: bool,
    /// A desktop id for `Terminal=true` launches; `None` means kitty.
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_terminal"
    )]
    pub terminal: Option<String>,
}

impl Default for ShellSettings {
    fn default() -> Self {
        Self {
            islands: Islands::default(),
            providers: Vec::new(),
            clock_24h: true,
            terminal: None,
        }
    }
}

fn valid_terminal(t: &str) -> bool {
    valid_pinned_id(t) && t.ends_with(".desktop")
}

fn expect_bool(key: &str, v: &Value) -> Result<bool, SettingError> {
    v.as_bool()
        .ok_or_else(|| invalid(key, "expected a boolean"))
}

impl ShellSettings {
    /// Every key of the `shell` section.
    pub const FIELDS: [&str; 4] = ["islands", "providers", "clock_24h", "terminal"];

    /// Rules shared by the API and the loader: ids valid, unique, at most 64, terminal valid.
    pub fn validate(&self) -> Result<(), SettingError> {
        if self.providers.len() > MAX_PROVIDERS {
            return Err(invalid(
                "shell.providers",
                format!("at most {MAX_PROVIDERS} entries"),
            ));
        }
        for (i, p) in self.providers.iter().enumerate() {
            if !ProviderPref::valid_id(&p.id) {
                return Err(invalid(
                    "shell.providers",
                    format!("invalid provider id {:?}", p.id),
                ));
            }
            if self.providers[..i].iter().any(|q| q.id == p.id) {
                return Err(invalid(
                    "shell.providers",
                    format!("duplicate provider id {:?}", p.id),
                ));
            }
        }
        if let Some(t) = &self.terminal
            && !valid_terminal(t)
        {
            return Err(invalid(
                "shell.terminal",
                "expected a desktop id ending in .desktop",
            ));
        }
        Ok(())
    }

    /// Load-time repair: drop invalid, duplicate and surplus providers, and an invalid
    /// terminal, instead of failing.
    pub fn sanitised(mut self) -> Self {
        let mut seen: Vec<String> = Vec::new();
        self.providers.retain(|p| {
            let keep = ProviderPref::valid_id(&p.id) && !seen.contains(&p.id);
            if keep {
                seen.push(p.id.clone());
            }
            keep
        });
        self.providers.truncate(MAX_PROVIDERS);
        self.terminal = self.terminal.filter(|t| valid_terminal(t));
        self
    }

    /// Parse a whole `shell` section strictly: unknown keys and wrong types are errors, and
    /// missing fields take their defaults.
    pub fn from_section(v: &Value) -> Result<Self, SettingError> {
        const KEY: &str = "shell";
        let obj = v
            .as_object()
            .ok_or_else(|| invalid(KEY, "expected an object"))?;
        check_known(KEY, v, &Self::FIELDS)?;
        let mut out = Self::default();
        if let Some(i) = obj.get("islands") {
            let io = i
                .as_object()
                .ok_or_else(|| invalid("shell.islands", "expected an object"))?;
            check_known("shell.islands", i, &Islands::FIELDS)?;
            for (name, slot) in [
                ("system", &mut out.islands.system),
                ("work", &mut out.islands.work),
                ("time", &mut out.islands.time),
                ("notif", &mut out.islands.notif),
                ("status", &mut out.islands.status),
            ] {
                if let Some(b) = io.get(name) {
                    *slot = expect_bool(&format!("shell.islands.{name}"), b)?;
                }
            }
        }
        if let Some(p) = obj.get("providers") {
            let arr = p
                .as_array()
                .ok_or_else(|| invalid("shell.providers", "expected an array"))?;
            for item in arr {
                check_known("shell.providers", item, &ProviderPref::FIELDS)?;
                let id = item
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("shell.providers", "each entry needs a string id"))?;
                let enabled = match item.get("enabled") {
                    None => true,
                    Some(b) => expect_bool("shell.providers.enabled", b)?,
                };
                out.providers.push(ProviderPref {
                    id: id.to_string(),
                    enabled,
                });
            }
        }
        if let Some(c) = obj.get("clock_24h") {
            out.clock_24h = expect_bool("shell.clock_24h", c)?;
        }
        match obj.get("terminal") {
            None | Some(Value::Null) => {}
            Some(Value::String(s)) => out.terminal = Some(s.clone()),
            Some(_) => return Err(invalid("shell.terminal", "expected a string or null")),
        }
        out.validate()?;
        Ok(out)
    }
}

/// The launcher's provider ids in display order: ids `prefs` lists first (in `prefs` order),
/// then the rest (builtins in [`BUILTIN_PROVIDERS`] order, then others sorted). Disabled ids
/// are dropped.
pub fn provider_order(prefs: &[ProviderPref], ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in prefs {
        if ids.contains(&p.id) && !out.contains(&p.id) {
            out.push(p.id.clone());
        }
    }
    let mut rest: Vec<&String> = ids.iter().filter(|i| !out.contains(i)).collect();
    rest.sort_by_key(|i| {
        (
            BUILTIN_PROVIDERS
                .iter()
                .position(|b| b == i)
                .unwrap_or(BUILTIN_PROVIDERS.len()),
            (*i).clone(),
        )
    });
    rest.dedup();
    out.extend(rest.into_iter().cloned());
    out.retain(|i| provider_enabled(prefs, i));
    out
}

/// True unless `id` is listed and disabled.
pub fn provider_enabled(prefs: &[ProviderPref], id: &str) -> bool {
    prefs.iter().find(|p| p.id == id).is_none_or(|p| p.enabled)
}

#[cfg(test)]
mod tests {
    use super::super::Settings;
    use super::*;
    use serde_json::json;

    fn pref(id: &str, enabled: bool) -> ProviderPref {
        ProviderPref {
            id: id.into(),
            enabled,
        }
    }

    #[test]
    fn defaults() {
        let s = ShellSettings::default();
        assert_eq!(s.islands, Islands::default());
        assert!(
            s.islands.system
                && s.islands.work
                && s.islands.time
                && s.islands.notif
                && s.islands.status
        );
        assert!(s.clock_24h && s.providers.is_empty() && s.terminal.is_none());
        assert_eq!(s, serde_json::from_value(json!({})).unwrap());
        assert_eq!(s, ShellSettings::from_section(&json!({})).unwrap());
        assert!(s.validate().is_ok());
    }

    #[test]
    fn rejects_bad_provider_ids_and_dupes() {
        let mut set = Settings::default();
        for id in [":1.5", "a", "org..x", ""] {
            let v = json!({"providers": [{"id": id}]});
            assert!(set.set_section("shell", v).is_err(), "{id}");
        }
        let dup = json!({"providers": [{"id": "calc"}, {"id": "calc", "enabled": false}]});
        assert!(set.set_section("shell", dup).is_err());
        let many: Vec<_> = (0..65)
            .map(|i| json!({"id": format!("org.x.P{i}")}))
            .collect();
        assert!(
            set.set_section("shell", json!({"providers": many}))
                .is_err()
        );
        assert_eq!(set.shell, ShellSettings::default(), "errors leave it alone");
        let ok = json!({"providers": [{"id": "run"}, {"id": "org.x.Y", "enabled": false}]});
        set.set_section("shell", ok).unwrap();
        assert_eq!(
            set.shell.providers,
            [pref("run", true), pref("org.x.Y", false)]
        );
        for bad in [
            json!({"nope": 1}),
            json!({"islands": {"nope": true}}),
            json!({"islands": {"system": "yes"}}),
            json!({"islands": 3}),
            json!({"providers": "calc"}),
            json!({"providers": [{"id": "calc", "x": 1}]}),
            json!({"providers": [{"enabled": true}]}),
            json!({"providers": [{"id": "calc", "enabled": 1}]}),
            json!({"clock_24h": "no"}),
            json!({"terminal": 5}),
            json!([]),
        ] {
            assert!(set.set_section("shell", bad.clone()).is_err(), "{bad}");
        }
    }

    #[test]
    fn terminal_must_be_desktop_id() {
        let mut set = Settings::default();
        for bad in ["kitty", "a b.desktop", "../x.desktop", "", ".desktop.x"] {
            assert!(
                set.set_section("shell", json!({"terminal": bad})).is_err(),
                "{bad}"
            );
        }
        set.set_section("shell", json!({"terminal": "foot.desktop"}))
            .unwrap();
        assert_eq!(set.shell.terminal.as_deref(), Some("foot.desktop"));
        set.set_section("shell", json!({"terminal": null})).unwrap();
        assert_eq!(set.shell.terminal, None);
        let v = set.section("shell").unwrap();
        assert_eq!(v["clock_24h"], true);
        assert_eq!(v["islands"]["status"], true);
    }

    #[test]
    fn provider_order_and_enabled() {
        let prefs = [pref("run", true), pref("org.x.Y", false)];
        let ids: Vec<String> = ["calc", "run", "org.x.Y", "org.a.B"]
            .map(String::from)
            .to_vec();
        assert_eq!(provider_order(&prefs, &ids), ["run", "calc", "org.a.B"]);
        assert!(provider_enabled(&prefs, "run"));
        assert!(!provider_enabled(&prefs, "org.x.Y"));
        assert!(provider_enabled(&prefs, "unlisted"));
        // Unlisted ids: builtins first in builtin order, then others sorted.
        let ids: Vec<String> = ["org.z.Z", "run", "org.a.B", "calc"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            provider_order(&[], &ids),
            ["calc", "run", "org.a.B", "org.z.Z"]
        );
    }

    #[test]
    fn load_is_lenient() {
        let s: ShellSettings = serde_json::from_value(json!({
            "islands": {"system": "x", "work": false, "future": 1},
            "providers": [{"id": "calc"}, 5, {"id": ":1.2"}, {"id": "calc"}, {"id": "org.x.Y", "enabled": "?"}],
            "clock_24h": 3, "terminal": "kitty"
        }))
        .unwrap();
        let s = s.sanitised();
        assert!(s.islands.system && !s.islands.work);
        assert_eq!(s.providers, [pref("calc", true), pref("org.x.Y", true)]);
        assert!(s.clock_24h);
        assert_eq!(s.terminal, None);
        let s: ShellSettings =
            serde_json::from_value(json!({"islands": 3, "providers": 4, "terminal": 1})).unwrap();
        assert_eq!(s, ShellSettings::default());
    }
}
