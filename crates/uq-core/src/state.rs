//! uqd's persisted desktop state (`~/.local/state/uq/state.toml`).

use crate::atomic::atomic_write;
use crate::autostart::Autostart;
use crate::compat::Compat;
use crate::env::Env;
use crate::lua::Layout;
use crate::rules::Rules;
use crate::schedule::Schedule;
use crate::settings::{LayoutOptions, SettingError, Settings};
use crate::theme::Variant;
use crate::welcome::Welcome;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const DEFAULT_THEME: &str = "graphite";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub theme: String,
    pub variant: Variant,
    pub schedule: Schedule,
    #[serde(deserialize_with = "lenient_rules")]
    pub layouts: BTreeMap<String, Layout>,
    /// Window, panel and workspace rules; each entry loads on its own.
    #[serde(default)]
    pub rules: Rules,
    /// Environment variables for the session; each entry loads on its own.
    #[serde(default)]
    pub env: Env,
    /// Apps and scaling: the switches and the per-app fixes; each fix loads on its own.
    #[serde(default)]
    pub compat: Compat,
    /// Startup apps: choices about the XDG entries, and the user's own; each loads on its own.
    #[serde(default)]
    pub autostart: Autostart,
    /// The scheduled transition uqd last applied; a manual change holds until a newer one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule_mark: Option<chrono::DateTime<chrono::Utc>>,
    /// Whether Welcome is still due (spec §5.1). Bookkeeping, not a setting: never journalled.
    #[serde(deserialize_with = "crate::welcome::lenient")]
    pub welcome: Welcome,
    #[serde(flatten)]
    pub settings: Settings,
    pub launch_counts: BTreeMap<String, u32>,
}

/// Workspace rules load one by one: a rule whose layout this version doesn't know (a newer
/// uQ's) is dropped with a log line, and the others are kept.
fn lenient_rules<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, Layout>, D::Error> {
    let serde_json::Value::Object(obj) = serde_json::Value::deserialize(d)? else {
        tracing::warn!("state: [layouts] is not a table; ignoring it");
        return Ok(BTreeMap::new());
    };
    Ok(obj
        .into_iter()
        .filter_map(
            |(ws, v)| match serde_json::from_value::<Layout>(v.clone()) {
                Ok(l) => Some((ws, l)),
                Err(_) => {
                    tracing::warn!("state: dropping workspace rule {ws:?} = {v}");
                    None
                }
            },
        )
        .collect())
}

/// TOML has no null, so a disabled idle stage (`None`) is stored as 0 in `state.toml`.
const IDLE_STAGES: [&str; 4] = ["dim", "lock", "screen_off", "suspend_battery"];

impl Default for State {
    fn default() -> Self {
        State {
            theme: DEFAULT_THEME.into(),
            variant: Variant::Dark,
            schedule: Schedule::Off,
            layouts: BTreeMap::new(),
            rules: Rules::default(),
            env: Env::default(),
            compat: Compat::default(),
            autostart: Autostart::default(),
            schedule_mark: None,
            welcome: Welcome::default(),
            settings: Settings::default(),
            launch_counts: BTreeMap::new(),
        }
    }
}

impl State {
    /// Load state. A missing file yields the defaults. A file that won't parse is renamed to
    /// `state.toml.bad-<unix_secs>` first so pins and counts are never silently lost. A value
    /// that is wrong-typed, or parses but breaks a rule, costs only its own field or domain
    /// (see [`Settings::validated`]); the file is kept.
    pub fn load(path: &Path) -> State {
        let src = match std::fs::read_to_string(path) {
            Ok(src) => src,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return State::default(),
            Err(e) => {
                tracing::warn!("{}: {e}; using defaults", path.display());
                return State::default();
            }
        };
        match Self::parse_strict(&src) {
            Ok(state) => state,
            Err(e) => {
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let mut bad = path.as_os_str().to_owned();
                bad.push(format!(".bad-{secs}"));
                match std::fs::rename(path, &bad) {
                    Ok(()) => tracing::warn!(
                        "{}: unreadable state ({e}); kept as {:?}, using defaults",
                        path.display(),
                        bad
                    ),
                    Err(re) => tracing::warn!(
                        "{}: unreadable state ({e}), could not move it aside ({re}); using defaults",
                        path.display()
                    ),
                }
                State::default()
            }
        }
    }

    /// Parse `state.toml` text without touching any file: a document that isn't valid TOML or
    /// doesn't fit `State` (wrong type, unknown variant) is an error. Values that parse but break
    /// a rule are repaired as [`State::load`] does, costing only their own field or domain.
    pub fn parse_strict(src: &str) -> Result<State, toml::de::Error> {
        let mut state = Self::parse(src)?;
        let (valid, repaired) = state.settings.validated();
        for e in repaired {
            tracing::warn!("state: {e}; that setting uses its default");
        }
        state.settings = valid;
        state.layouts = crate::settings::repair_rules(std::mem::take(&mut state.layouts));
        state.rules = std::mem::take(&mut state.rules).repaired();
        state.env = std::mem::take(&mut state.env).repaired();
        state.compat = std::mem::take(&mut state.compat).repaired();
        state.autostart = std::mem::take(&mut state.autostart).repaired();
        Ok(state)
    }

    fn parse(src: &str) -> Result<State, toml::de::Error> {
        let table: toml::Table = toml::from_str(src)?;
        let mut state: State = table.clone().try_into()?;
        if let Some(idle) = table.get("idle").and_then(toml::Value::as_table) {
            for stage in IDLE_STAGES {
                if idle.get(stage).and_then(toml::Value::as_integer) == Some(0) {
                    *Self::stage_mut(&mut state.settings, stage) = None;
                }
            }
        }
        Ok(state)
    }

    fn stage_mut<'a>(s: &'a mut Settings, stage: &str) -> &'a mut Option<u32> {
        match stage {
            "dim" => &mut s.idle.dim,
            "lock" => &mut s.idle.lock,
            "screen_off" => &mut s.idle.screen_off,
            _ => &mut s.idle.suspend_battery,
        }
    }

    /// One settings section as JSON. `layouts` joins the workspace rules in `self.layouts` with
    /// `settings.layout_options`; every other domain is `Settings`'.
    pub fn section(&self, domain: &str) -> Result<serde_json::Value, SettingError> {
        if domain == "rules" {
            return Ok(serde_json::to_value(&self.rules).expect("rules serialise"));
        }
        if domain == "env" {
            return Ok(self.env.section());
        }
        if domain == "compat" {
            return Ok(self.compat.section());
        }
        if domain == "autostart" {
            return Ok(self.autostart.section());
        }
        if domain != "layouts" {
            return self.settings.section(domain);
        }
        let mut v =
            serde_json::to_value(&self.settings.layout_options).expect("settings serialise");
        v["workspaces"] = serde_json::to_value(&self.layouts).expect("layouts serialise");
        Ok(v)
    }

    /// Replace a whole section; on error `self` is unchanged. A `layouts` write replaces both
    /// the options and every workspace rule (selectors normalised); a missing `workspaces`
    /// clears the rules.
    pub fn set_section(
        &mut self,
        domain: &str,
        value: serde_json::Value,
    ) -> Result<(), SettingError> {
        if domain == "rules" {
            self.rules = Rules::from_section(&value, &self.rules)?;
            return Ok(());
        }
        if domain == "env" {
            self.env = Env::from_section(&value)?;
            return Ok(());
        }
        if domain == "compat" {
            self.compat = Compat::from_section(&value)?;
            return Ok(());
        }
        if domain == "autostart" {
            self.autostart = Autostart::from_section(&value, &self.autostart)?;
            return Ok(());
        }
        if domain != "layouts" {
            return self.settings.set_section(domain, value);
        }
        let (options, rules) = LayoutOptions::from_section(&value)?;
        self.settings.layout_options = options;
        self.layouts = rules;
        Ok(())
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut table = toml::Table::try_from(self).map_err(std::io::Error::other)?;
        let mut settings = self.settings.clone();
        if let Some(idle) = table.get_mut("idle").and_then(toml::Value::as_table_mut) {
            for stage in IDLE_STAGES {
                if Self::stage_mut(&mut settings, stage).is_none() {
                    idle.insert(stage.into(), toml::Value::Integer(0));
                }
            }
        }
        let body = toml::to_string(&table).map_err(std::io::Error::other)?;
        atomic_write(path, &body)
    }
}

#[cfg(test)]
mod load_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_graphite_dark() {
        let s = State::default();
        assert_eq!((s.theme.as_str(), s.variant), ("graphite", Variant::Dark));
        assert_eq!(s.schedule, Schedule::Off);
        assert!(s.layouts.is_empty());
    }

    #[test]
    fn missing_state_is_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(State::load(&dir.path().join("nope.toml")), State::default());
    }

    #[test]
    fn corrupt_state_is_renamed() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        std::fs::write(&p, "{{nope").unwrap();
        assert_eq!(State::load(&p), State::default());
        assert!(!p.exists());
        let bad: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|f| {
                f.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("state.toml.bad-")
            })
            .collect();
        assert_eq!(bad.len(), 1, "{bad:?}");
        assert_eq!(std::fs::read(&bad[0]).unwrap(), b"{{nope");
    }

    #[test]
    fn phase1_state_upgrades() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        std::fs::write(&p, include_str!("../tests/fixtures/phase1-state.toml")).unwrap();
        let s = State::load(&p);
        assert_eq!(s.settings, Settings::default());
        assert!(s.launch_counts.is_empty());
        assert_eq!(s.theme, "dracula");
        assert_eq!(s.variant, Variant::Light);
        assert_eq!(s.layouts.get("2"), Some(&Layout::Scrolling));
        assert_eq!(s.layouts.get("special:magic"), Some(&Layout::Master));
        assert!(p.exists(), "a valid Phase 1 file is not renamed");
    }

    #[test]
    fn out_of_order_idle_on_load_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        std::fs::write(
            &p,
            "launch_counts = { a = 3 }\n[idle]\ndim = 500\nlock = 300\n",
        )
        .unwrap();
        let s = State::load(&p);
        assert_eq!(s.settings, Settings::default());
        assert_eq!(s.launch_counts.get("a"), Some(&3), "other state is kept");
        assert!(p.exists(), "only unparsable files are renamed");
    }

    #[test]
    fn disabled_idle_stage_and_settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        let mut s = State::default();
        s.settings.idle.dim = None;
        s.settings.weather_location = Some(crate::settings::Location {
            name: "London".into(),
            lat: 51.51,
            lon: -0.13,
        });
        s.settings.pinned = vec!["firefox.desktop".into()];
        s.launch_counts.insert("firefox.desktop".into(), 7);
        s.save(&p).unwrap();
        assert_eq!(State::load(&p), s);
    }

    #[test]
    fn state_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("uq/state.toml");
        let s = State {
            theme: "rosewood".into(),
            variant: Variant::Light,
            schedule: Schedule::Sun {
                lat: 51.5,
                lon: -0.13,
            },
            layouts: BTreeMap::from([
                ("2".into(), Layout::Scrolling),
                ("special:magic".into(), Layout::Master),
            ]),
            rules: Rules::default(),
            env: Env {
                vars: vec![crate::env::Var {
                    name: "GDK_BACKEND".into(),
                    value: "x11".into(),
                }],
            },
            compat: Compat {
                sharp_x11: true,
                apps: vec![crate::compat::AppFix {
                    scale: Some(1.5),
                    ..crate::compat::AppFix::new("code.desktop")
                }],
                ..Compat::default()
            },
            autostart: Autostart {
                files: vec![crate::autostart::FileChoice {
                    id: "nm-applet.desktop".into(),
                    enabled: Some(false),
                    delay: None,
                }],
                added: vec![crate::autostart::Added::Command {
                    id: "a-0b12".into(),
                    name: "Backup check".into(),
                    argv: vec!["backup-check".into(), "--quiet".into()],
                    enabled: true,
                    delay: Some(10),
                }],
            },
            schedule_mark: None,
            welcome: Welcome {
                done: true,
                version: 1,
            },
            settings: Settings::default(),
            launch_counts: BTreeMap::new(),
        };
        s.save(&p).unwrap();
        assert_eq!(State::load(&p), s);
    }

    #[test]
    fn state_without_welcome_loads_as_pending() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        std::fs::write(&p, include_str!("../tests/fixtures/phase1-state.toml")).unwrap();
        let s = State::load(&p);
        assert_eq!(s.welcome, Welcome::default());
        assert!(s.welcome.pending());
        assert_eq!(s.theme, "dracula", "the rest of the file loads");
        // A wrong-typed [welcome] costs only itself.
        std::fs::write(&p, "theme = \"rosewood\"\nwelcome = \"done\"\n").unwrap();
        let s = State::load(&p);
        assert_eq!(
            (s.theme.as_str(), s.welcome),
            ("rosewood", Welcome::default())
        );
        assert!(p.exists(), "not moved aside");
    }

    #[test]
    fn welcome_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        let mut s = State::default();
        s.welcome = Welcome {
            done: true,
            version: crate::welcome::CURRENT_WELCOME_VERSION,
        };
        s.save(&p).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains("[welcome]"), "{text}");
        assert_eq!(State::load(&p), s);
    }

    #[test]
    fn old_state_loads_with_default_sections() {
        let st = State::parse("theme = \"graphite\"\nvariant = \"dark\"\n").unwrap();
        assert_eq!(
            st.settings.appearance,
            crate::settings::Appearance::default()
        );
        assert_eq!(st.settings.wallpaper, crate::settings::Wallpaper::default());
    }

    #[test]
    fn null_overrides_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.toml");
        let mut st = State::default();
        st.settings
            .set_section(
                "appearance",
                serde_json::json!({"ui_font": null, "cursor_size": 32}),
            )
            .unwrap();
        st.settings
            .set_section(
                "wallpaper",
                serde_json::json!({"outputs": {"DP-1": {"image": null, "color": "#101010"}}}),
            )
            .unwrap();
        st.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            !text.contains("ui_font") && !text.contains("image"),
            "{text}"
        );
        assert_eq!(State::load(&path), st);
    }

    #[test]
    fn unknown_section_keys_and_fit_keep_state() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        std::fs::write(
            &p,
            "theme = \"rosewood\"\npinned = [\"firefox.desktop\"]\n\
             [appearance]\ncursor_size = 32\nfuture_key = 1\n\
             [wallpaper.outputs.\"*\"]\nfit = \"stretch\"\nshiny = true\n\
             [wallpaper]\nlater = \"x\"\n",
        )
        .unwrap();
        let s = State::load(&p);
        assert_eq!(s.theme, "rosewood");
        assert_eq!(s.settings.pinned, ["firefox.desktop"]);
        assert_eq!(s.settings.appearance.cursor_size, Some(32));
        assert_eq!(
            s.settings.wallpaper.outputs["*"].fit,
            crate::settings::Fit::Fill,
            "an unknown fit loads as fill"
        );
        assert!(p.exists(), "a file with unknown keys is not moved aside");
    }

    #[test]
    fn wallpaper_state_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        let mut st = State::default();
        st.settings
            .set_section(
                "wallpaper",
                serde_json::json!({"outputs": {
                    "*": {"image": "/w/a.png", "fit": "center"},
                    "DP-1": {"folder": "/w/slides", "interval": 60, "fit": "tile"},
                }}),
            )
            .unwrap();
        st.save(&p).unwrap();
        assert_eq!(State::load(&p), st);
    }

    #[test]
    fn layouts_section_normalises_and_replaces_rules() {
        let mut st = State::default();
        st.layouts.insert("2".into(), Layout::Scrolling);
        st.set_section(
            "layouts",
            serde_json::json!({"default": "master", "workspaces": {"web": "master"},
                "master": {"mfact": 0.62}}),
        )
        .unwrap();
        assert_eq!(
            st.layouts,
            BTreeMap::from([("name:web".into(), Layout::Master)])
        );
        let v = st.section("layouts").unwrap();
        assert_eq!(v["workspaces"], serde_json::json!({"name:web": "master"}));
        assert_eq!(v["default"], "master");
        assert_eq!(v["master"]["mfact"], 0.62);
        assert_eq!(v["dwindle"], serde_json::json!({}));
        // A missing `workspaces` means none.
        st.set_section("layouts", serde_json::json!({})).unwrap();
        assert!(st.layouts.is_empty());
        assert_eq!(st.settings.layout_options, LayoutOptions::default());
    }

    #[test]
    fn layouts_section_rejects_bad_input() {
        let mut st = State::default();
        st.set_section(
            "layouts",
            serde_json::json!({"workspaces": {"2": "dwindle"}}),
        )
        .unwrap();
        let before = st.clone();
        let many: serde_json::Map<_, _> = (1..=65)
            .map(|i| (i.to_string(), serde_json::json!("dwindle")))
            .collect();
        for bad in [
            serde_json::json!({"workspaces": many}),
            serde_json::json!({"workspaces": {"web": "monocle"}}),
            serde_json::json!({"workspaces": {"a\nb": "dwindle"}}),
            serde_json::json!({"workspaces": {"name:": "dwindle"}, "extra": 1}),
            serde_json::json!({"workspaces": {"a".repeat(65): "dwindle"}}),
            serde_json::json!({"workspaces": {"web": "master", "name:web": "dwindle"}}),
            serde_json::json!({"master": {"mfact": 0.95}}),
        ] {
            assert!(st.set_section("layouts", bad.clone()).is_err(), "{bad}");
            assert_eq!(st, before);
        }
        let mut many = serde_json::Map::new();
        for i in 1..=64 {
            many.insert(i.to_string(), serde_json::json!("dwindle"));
        }
        st.set_section("layouts", serde_json::json!({"workspaces": many}))
            .unwrap();
        assert_eq!(st.layouts.len(), 64);
        assert!(st.set_section("bogus", serde_json::json!({})).is_err());
    }

    #[test]
    fn old_state_keeps_workspace_rules() {
        let st = State::parse("theme = \"graphite\"\n[layouts]\n\"2\" = \"scrolling\"\n").unwrap();
        let v = st.section("layouts").unwrap();
        assert_eq!(v["workspaces"]["2"], "scrolling");
        assert_eq!(st.settings.look, crate::settings::Look::default());
        assert_eq!(st.settings.layout_options, LayoutOptions::default());
    }

    #[test]
    fn unknown_look_and_layout_values_keep_state() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        std::fs::write(
            &p,
            "theme = \"rosewood\"\npinned = [\"firefox.desktop\"]\n\
             [look]\ngaps_in = 4\nfuture_key = 1\n\
             [layout_options]\ndefault = \"monocle\"\nnew_key = 2\n\
             [layout_options.dwindle]\nforce_split = \"sideways\"\npreserve_split = true\n\
             [layout_options.master]\norientation = \"diagonal\"\nmfact = 0.6\n\
             [layouts]\n\"2\" = \"scrolling\"\n",
        )
        .unwrap();
        let s = State::load(&p);
        assert_eq!(s.theme, "rosewood");
        assert_eq!(s.settings.pinned, ["firefox.desktop"]);
        assert_eq!(s.settings.look.gaps_in, Some(4));
        let lo = &s.settings.layout_options;
        assert_eq!(
            (lo.default, lo.dwindle.force_split, lo.master.orientation),
            (None, None, None)
        );
        assert_eq!(
            (lo.dwindle.preserve_split, lo.master.mfact),
            (Some(true), Some(0.6))
        );
        assert_eq!(s.layouts.get("2"), Some(&Layout::Scrolling));
        assert!(
            p.exists(),
            "unknown keys or values do not move the file aside"
        );
    }

    #[test]
    fn bad_shell_keeps_state() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        for shell in [
            "shell = 5
",
            "[shell]
islands = 3
providers = [1, \"x\"]
clock_24h = \"no\"\nterminal = 7
",
            "[shell]
providers = [{id = \":1.5\"}, {id = \"calc\"}, {id = \"calc\"}]
terminal = \"kitty\"
[shell.islands]\nwork = \"x\"\nfuture = 1
",
        ] {
            std::fs::write(
                &p,
                format!("theme = \"rosewood\"\npinned = [\"a.desktop\"]\n{shell}"),
            )
            .unwrap();
            let s = State::load(&p);
            assert_eq!(s.theme, "rosewood", "{shell}");
            assert_eq!(s.settings.pinned, ["a.desktop"], "{shell}");
            assert!(p.exists());
            assert!(s.settings.shell.islands.work && s.settings.shell.clock_24h);
            assert_eq!(s.settings.shell.terminal, None);
            assert!(s.settings.shell.providers.len() <= 1);
        }
    }

    #[test]
    fn shell_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        let mut st = State::default();
        st.set_section(
            "shell",
            serde_json::json!({"islands": {"time": false}, "clock_24h": false,
                "terminal": "foot.desktop",
                "providers": [{"id": "run"}, {"id": "org.x.Y", "enabled": false}]}),
        )
        .unwrap();
        st.save(&p).unwrap();
        assert_eq!(State::load(&p), st);
    }

    #[test]
    fn look_and_layout_options_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        let mut st = State::default();
        st.set_section(
            "look",
            serde_json::json!({"gaps_in": 3, "blur": false, "animation_speed": 1.5}),
        )
        .unwrap();
        st.set_section(
            "layouts",
            serde_json::json!({"default": "scrolling", "workspaces": {"2": "master"},
                "dwindle": {"force_split": "right"}, "master": {"new_status": "slave", "orientation": "top"},
                "scrolling": {"column_width": 0.7, "fullscreen_on_one_column": true}}),
        )
        .unwrap();
        st.save(&p).unwrap();
        assert_eq!(State::load(&p), st);
    }
}
