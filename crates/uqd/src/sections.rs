//! Whole-section settings (`appearance`, `wallpaper`) for `GetSection` / `SetSection`.

use crate::service::{Change, Desktop, DesktopError, Event};
use uq_core::history::Origin;
use uq_core::settings::renders_section;

impl Desktop {
    /// One settings section as JSON (`appearance` or `wallpaper`).
    pub fn get_section(&self, domain: &str) -> Result<String, DesktopError> {
        let v = self
            .state
            .section(domain)
            .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
        serde_json::to_string(&v).map_err(|e| DesktopError::Io(e.to_string()))
    }

    /// Replace a whole section. `appearance` re-renders through the normal commit (verify,
    /// reload, theme events); `look` and `layouts` render, verify and reload Hyprland only;
    /// `wallpaper` and `shell` just save, since the shell reads them.
    pub async fn set_section(
        &mut self,
        domain: &str,
        json: &str,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        let value: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| DesktopError::InvalidSetting(format!("value is not JSON: {e}")))?;
        let mut next = self.state.clone();
        next.set_section(domain, value)
            .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
        let before = (domain == "layouts").then(|| self.state.layouts.clone());
        if renders_section(domain) {
            let change = if domain == "appearance" {
                Change::Theme
            } else {
                Change::Reload
            };
            self.commit(next, change, origin).await?;
        } else {
            self.persist(next, origin)?;
        }
        if let Some(before) = before {
            // The reload applied the rules; tell the shell which workspaces changed layout. A
            // removed rule falls back to the default layout.
            let fallback = self
                .state
                .settings
                .layout_options
                .default
                .unwrap_or(uq_core::lua::Layout::Dwindle);
            let mut changed: Vec<_> = self
                .state
                .layouts
                .iter()
                .filter(|(ws, l)| before.get(*ws) != Some(*l))
                .map(|(ws, l)| (ws.clone(), *l))
                .collect();
            changed.extend(
                before
                    .keys()
                    .filter(|ws| !self.state.layouts.contains_key(*ws))
                    .map(|ws| (ws.clone(), fallback)),
            );
            for (workspace, layout) in changed {
                let _ = self.events.send(Event::LayoutChanged { workspace, layout });
            }
        }
        let section = self.get_section(domain)?;
        let _ = self.events.send(Event::SectionChanged {
            domain: domain.to_string(),
            json: section,
        });
        let _ = self.events.send(Event::SettingsChanged(Box::new(
            self.state.settings.clone(),
        )));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::fake::FakeCompositor;
    use crate::system::FakeSystem;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use tokio::sync::broadcast;
    use uq_core::history::Origin;
    use uq_core::paths::Paths;
    use uq_core::state::State;

    struct Rig {
        _dir: tempfile::TempDir,
        desktop: Desktop,
        comp: Arc<FakeCompositor>,
        sys: Arc<FakeSystem>,
        events: broadcast::Receiver<Event>,
    }

    fn rig() -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut paths = Paths::under(dir.path());
        paths.themes_dir = repo.join("themes");
        paths.templates_dir = repo.join("templates");
        let kv = dir.path().join("kvantum");
        std::fs::create_dir_all(kv.join("KvGnome")).unwrap();
        std::fs::write(kv.join("KvGnome/KvGnome.kvconfig"), "").unwrap();
        let comp = Arc::new(FakeCompositor::running());
        let sys = Arc::new(FakeSystem::default());
        let desktop = Desktop::new(paths, comp.clone(), sys.clone())
            .unwrap()
            .with_kvantum_dir(kv);
        let events = desktop.subscribe();
        Rig {
            _dir: dir,
            desktop,
            comp,
            sys,
            events,
        }
    }

    fn drain(rx: &mut broadcast::Receiver<Event>) -> Vec<Event> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    fn kvconfig(r: &Rig) -> PathBuf {
        r.desktop.paths().theme_dir.join("Kvantum/uq/uq.kvconfig")
    }

    #[tokio::test]
    async fn set_appearance_renders_overrides_and_sets_cursor() {
        let mut r = rig();
        r.desktop
            .set_section(
                "appearance",
                r#"{"ui_font":"Cantarell","cursor_theme":"Adwaita","cursor_size":32}"#,
                Origin::Settings,
            )
            .await
            .unwrap();
        let tokens =
            std::fs::read_to_string(r.desktop.paths().theme_dir.join("tokens.json")).unwrap();
        assert!(tokens.contains("Cantarell"), "{tokens}");
        assert!(r.comp.calls().contains(&"setcursor Adwaita 32".to_string()));
        assert!(kvconfig(&r).is_file());
        assert_eq!(
            State::load(&r.desktop.paths().state_file)
                .settings
                .appearance
                .ui_font
                .as_deref(),
            Some("Cantarell")
        );
        let evs = drain(&mut r.events);
        assert!(evs.iter().any(|e| matches!(e, Event::ThemeChanged { .. })));
        assert!(
            evs.iter().any(
                |e| matches!(e, Event::SectionChanged { domain, .. } if domain == "appearance")
            )
        );
    }

    #[tokio::test]
    async fn appearance_commit_survives_gsettings_failure() {
        let mut r = rig();
        r.sys.fail.store(true, Ordering::SeqCst);
        r.desktop
            .set_section("appearance", r#"{"mono_font":"Hack"}"#, Origin::Settings)
            .await
            .unwrap();
        let got: serde_json::Value =
            serde_json::from_str(&r.desktop.get_section("appearance").unwrap()).unwrap();
        assert_eq!(got["mono_font"], "Hack");
        assert!(r.sys.applied.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn kvantum_off_removes_the_kvconfig() {
        let mut r = rig();
        r.desktop
            .set_section("appearance", "{}", Origin::Settings)
            .await
            .unwrap();
        assert!(kvconfig(&r).is_file());
        r.desktop
            .set_section("appearance", r#"{"kvantum":false}"#, Origin::Settings)
            .await
            .unwrap();
        assert!(!kvconfig(&r).exists());
    }

    #[tokio::test]
    async fn set_wallpaper_saves_and_emits_without_render() {
        let mut r = rig();
        r.desktop.startup().await;
        let gen_path = r.desktop.paths().generated_lua.clone();
        let before = std::fs::metadata(&gen_path).unwrap().modified().unwrap();
        r.comp.calls.lock().unwrap().clear();
        drain(&mut r.events);
        let json = r#"{"outputs":{"*":{"image":"/a.png"}}}"#;
        r.desktop
            .set_section("wallpaper", json, Origin::Settings)
            .await
            .unwrap();
        assert_eq!(
            std::fs::metadata(&gen_path).unwrap().modified().unwrap(),
            before
        );
        assert!(r.comp.calls().is_empty(), "{:?}", r.comp.calls());
        let evs = drain(&mut r.events);
        assert!(
            evs.iter().any(
                |e| matches!(e, Event::SectionChanged { domain, .. } if domain == "wallpaper")
            )
        );
        assert!(evs.iter().any(|e| matches!(e, Event::SettingsChanged(_))));
        assert!(!evs.iter().any(|e| matches!(e, Event::ThemeChanged { .. })));
        let saved = State::load(&r.desktop.paths().state_file);
        assert!(saved.settings.wallpaper.outputs.contains_key("*"));
    }

    #[tokio::test]
    async fn set_shell_saves_and_emits_without_render() {
        let mut r = rig();
        r.desktop.startup().await;
        let gen_path = r.desktop.paths().generated_lua.clone();
        let before = std::fs::metadata(&gen_path).unwrap().modified().unwrap();
        r.comp.calls.lock().unwrap().clear();
        drain(&mut r.events);
        let json = r#"{"islands":{"work":false},"clock_24h":false,"terminal":"foot.desktop","providers":[{"id":"run"},{"id":"org.x.Y","enabled":false}]}"#;
        r.desktop
            .set_section("shell", json, Origin::Settings)
            .await
            .unwrap();
        assert_eq!(
            std::fs::metadata(&gen_path).unwrap().modified().unwrap(),
            before
        );
        assert!(r.comp.calls().is_empty(), "{:?}", r.comp.calls());
        let evs = drain(&mut r.events);
        assert!(
            evs.iter()
                .any(|e| matches!(e, Event::SectionChanged { domain, .. } if domain == "shell"))
        );
        assert!(evs.iter().any(|e| matches!(e, Event::SettingsChanged(_))));
        assert!(!evs.iter().any(|e| matches!(e, Event::ThemeChanged { .. })));
        let saved = State::load(&r.desktop.paths().state_file);
        let sh = &saved.settings.shell;
        assert!(!sh.islands.work && !sh.clock_24h && sh.islands.time);
        assert_eq!(sh.terminal.as_deref(), Some("foot.desktop"));
        assert_eq!(sh.providers.len(), 2);
        let got: serde_json::Value =
            serde_json::from_str(&r.desktop.get_section("shell").unwrap()).unwrap();
        assert_eq!(got["providers"][1]["enabled"], false);
        for bad in [r#"{"terminal":"kitty"}"#, r#"{"nope":1}"#] {
            assert!(matches!(
                r.desktop.set_section("shell", bad, Origin::Settings).await,
                Err(DesktopError::InvalidSetting(_))
            ));
        }
    }

    #[tokio::test]
    async fn invalid_section_is_rejected_and_state_unchanged() {
        let mut r = rig();
        let before = r.desktop.get_section("appearance").unwrap();
        for (d, j) in [
            ("appearance", r#"{"font_size":5}"#),
            ("appearance", "not json"),
            ("wallpaper", r#"{"outputs":{"*":{"image":"../x"}}}"#),
            ("nope", "{}"),
        ] {
            assert!(
                matches!(
                    r.desktop.set_section(d, j, Origin::Settings).await,
                    Err(DesktopError::InvalidSetting(_))
                ),
                "{d} {j}"
            );
        }
        assert_eq!(r.desktop.get_section("appearance").unwrap(), before);
        assert!(r.desktop.get_section("nope").is_err());
        assert!(drain(&mut r.events).is_empty());
    }

    #[tokio::test]
    async fn rejected_config_keeps_previous_appearance() {
        let mut r = rig();
        r.desktop
            .set_section("appearance", r#"{"ui_font":"Inter"}"#, Origin::Settings)
            .await
            .unwrap();
        let before = r.desktop.get_section("appearance").unwrap();
        drain(&mut r.events);
        *r.comp.verify_error.lock().unwrap() = Some("bad".into());
        let res = r
            .desktop
            .set_section("appearance", r#"{"ui_font":"Other"}"#, Origin::Settings)
            .await;
        assert!(matches!(res, Err(DesktopError::ConfigInvalid(_))));
        assert_eq!(r.desktop.get_section("appearance").unwrap(), before);
        let saved = State::load(&r.desktop.paths().state_file);
        assert_eq!(saved.settings.appearance.ui_font.as_deref(), Some("Inter"));
        let evs = drain(&mut r.events);
        assert!(evs.iter().all(|e| matches!(e, Event::ConfigError(_))));
    }

    fn generated(r: &Rig) -> String {
        std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap()
    }

    #[tokio::test]
    async fn set_look_renders_and_reloads() {
        let mut r = rig();
        r.desktop.startup().await;
        r.comp.calls.lock().unwrap().clear();
        r.sys.applied.lock().unwrap().clear();
        drain(&mut r.events);
        r.desktop
            .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
            .await
            .unwrap();
        assert!(generated(&r).contains("gaps_in = 9"), "{}", generated(&r));
        let calls = r.comp.calls();
        assert!(calls.contains(&"reload".to_string()), "{calls:?}");
        assert!(
            !calls.iter().any(|c| c.starts_with("setcursor")),
            "{calls:?}"
        );
        assert!(r.sys.applied.lock().unwrap().is_empty());
        let evs = drain(&mut r.events);
        assert!(!evs.iter().any(|e| matches!(e, Event::ThemeChanged { .. })));
        assert!(
            evs.iter()
                .any(|e| matches!(e, Event::SectionChanged { domain, .. } if domain == "look"))
        );
        assert!(evs.iter().any(|e| matches!(e, Event::SettingsChanged(_))));
        let saved = State::load(&r.desktop.paths().state_file);
        assert_eq!(saved.settings.look.gaps_in, Some(9));
    }

    #[tokio::test]
    async fn rejected_look_keeps_previous_config() {
        let mut r = rig();
        r.desktop.startup().await;
        let gen_before = std::fs::read(&r.desktop.paths().generated_lua).unwrap();
        let state_before = std::fs::read(&r.desktop.paths().state_file).unwrap();
        drain(&mut r.events);
        *r.comp.verify_error.lock().unwrap() = Some("bad option".into());
        let res = r
            .desktop
            .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
            .await;
        assert_eq!(res, Err(DesktopError::ConfigInvalid("bad option".into())));
        assert_eq!(
            std::fs::read(&r.desktop.paths().generated_lua).unwrap(),
            gen_before
        );
        // startup already wrote state.toml (with an empty `[look]` table); it must be untouched.
        let state = std::fs::read(&r.desktop.paths().state_file).unwrap();
        assert_eq!(state, state_before);
        assert!(!String::from_utf8(state).unwrap().contains("gaps_in"));
        let evs = drain(&mut r.events);
        assert!(
            evs.iter().all(|e| matches!(e, Event::ConfigError(_))),
            "{evs:?}"
        );
        let look: serde_json::Value =
            serde_json::from_str(&r.desktop.get_section("look").unwrap()).unwrap();
        assert!(look["gaps_in"].is_null(), "{look}");
    }

    #[tokio::test]
    async fn set_layouts_replaces_rules() {
        let mut r = rig();
        r.desktop
            .set_layout("2", "scrolling", Origin::Settings)
            .await
            .unwrap();
        r.comp.calls.lock().unwrap().clear();
        drain(&mut r.events);
        r.desktop
            .set_section(
                "layouts",
                r#"{"default":"master","workspaces":{"3":"master"}}"#,
                Origin::Settings,
            )
            .await
            .unwrap();
        let g = generated(&r);
        assert!(g.contains(r#"workspace = "3", layout = "master""#), "{g}");
        assert!(!g.contains(r#"workspace = "2""#), "{g}");
        assert!(r.comp.calls().contains(&"reload".to_string()));
        let saved = State::load(&r.desktop.paths().state_file);
        assert_eq!(
            saved.layouts.into_iter().collect::<Vec<_>>(),
            vec![("3".to_string(), uq_core::lua::Layout::Master)]
        );
        let evs = drain(&mut r.events);
        assert!(evs.contains(&Event::LayoutChanged {
            workspace: "3".into(),
            layout: uq_core::lua::Layout::Master
        }));
        assert!(
            evs.iter()
                .any(|e| matches!(e, Event::SectionChanged { domain, .. } if domain == "layouts"))
        );
        // The removed rule "2" reports the new default.
        assert!(evs.contains(&Event::LayoutChanged {
            workspace: "2".into(),
            layout: uq_core::lua::Layout::Master
        }));
        assert!(!evs.iter().any(|e| matches!(e, Event::ThemeChanged { .. })));
    }

    #[tokio::test]
    async fn set_layout_emits_layouts_section() {
        let mut r = rig();
        r.desktop
            .set_layout("2", "scrolling", Origin::Settings)
            .await
            .unwrap();
        let evs = drain(&mut r.events);
        let json = evs
            .iter()
            .find_map(|e| match e {
                Event::SectionChanged { domain, json } if domain == "layouts" => Some(json),
                _ => None,
            })
            .expect("SectionChanged layouts");
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(v["workspaces"]["2"], "scrolling");
    }
}
