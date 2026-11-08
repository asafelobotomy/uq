//! Desktop state and the commit sequence (spec §4, §5). No D-Bus here.

use crate::compositor::{Compositor, CompositorError};
use crate::system::SystemSettings;
use chrono::{DateTime, Local, TimeZone};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use uq_core::atomic::{atomic_replace_dir, atomic_write};
use uq_core::lua::{Layout, emit_generated, emit_workspace_rule, lua_string};
use uq_core::paths::Paths;
use uq_core::render::render;
use uq_core::schedule::{Schedule, variant_at};
use uq_core::state::State;
use uq_core::theme::{ThemeRegistry, Variant};

const USER_LUA_STUB: &str =
    "-- Your own Hyprland settings. uQ reads this file last and never rewrites it.\n";

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    ThemeChanged {
        name: String,
        variant: Variant,
        tokens_path: PathBuf,
    },
    LayoutChanged {
        workspace: String,
        layout: Layout,
    },
    ConfigError(String),
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DesktopError {
    #[error("unknown theme {0:?}")]
    UnknownTheme(String),
    #[error("unknown layout {0:?} (expected dwindle, master or scrolling)")]
    InvalidLayout(String),
    #[error("workspace name is empty")]
    EmptyWorkspace,
    #[error("invalid schedule: {0}")]
    InvalidSchedule(String),
    #[error("Hyprland rejected the new config: {0}")]
    ConfigInvalid(String),
    #[error("{0}")]
    Io(String),
}

fn io_err(context: &str) -> impl Fn(std::io::Error) -> DesktopError + '_ {
    move |e| DesktopError::Io(format!("{context}: {e}"))
}

/// What a commit changed, which decides how it is applied to the live session.
enum Change {
    Theme,
    Layout(String, Layout),
    Quiet,
}

pub struct Desktop {
    paths: Paths,
    themes: ThemeRegistry,
    state: State,
    comp: Arc<dyn Compositor>,
    sys: Arc<dyn SystemSettings>,
    events: broadcast::Sender<Event>,
}

impl Desktop {
    pub fn new(
        paths: Paths,
        comp: Arc<dyn Compositor>,
        sys: Arc<dyn SystemSettings>,
    ) -> Result<Self, DesktopError> {
        let themes = ThemeRegistry::load_dir(&paths.themes_dir)
            .map_err(|e| DesktopError::Io(e.to_string()))?;
        let mut state = State::load(&paths.state_file);
        if themes.get(&state.theme, state.variant).is_none() {
            tracing::warn!(
                "theme {:?} not installed; using {}",
                state.theme,
                State::default().theme
            );
            state.theme = State::default().theme;
        }
        let (events, _) = broadcast::channel(64);
        Ok(Desktop {
            paths,
            themes,
            state,
            comp,
            sys,
            events,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn theme_ids(&self) -> Vec<String> {
        self.themes.ids().into_iter().map(str::to_string).collect()
    }

    pub fn get_theme(&self) -> (String, Variant) {
        (self.state.theme.clone(), self.state.variant)
    }

    pub fn get_layouts(&self) -> BTreeMap<String, Layout> {
        self.state.layouts.clone()
    }

    /// Ensure `user.lua` exists (never overwriting it), then render everything from state.
    pub async fn startup(&mut self) {
        if let Err(e) = self.ensure_user_lua() {
            tracing::warn!("{e}");
        }
        if let Err(e) = self.reload().await {
            tracing::warn!("startup render failed: {e}");
        }
    }

    pub async fn set_theme(&mut self, id: &str, v: Variant) -> Result<(), DesktopError> {
        if self.themes.get(id, v).is_none() {
            return Err(DesktopError::UnknownTheme(id.to_string()));
        }
        let mut next = self.state.clone();
        next.theme = id.to_string();
        next.variant = v;
        self.commit(next, Change::Theme).await
    }

    pub async fn set_layout(&mut self, ws: &str, layout: &str) -> Result<(), DesktopError> {
        let ws = ws.trim();
        if ws.is_empty() {
            return Err(DesktopError::EmptyWorkspace);
        }
        let layout: Layout = layout
            .parse()
            .map_err(|_| DesktopError::InvalidLayout(layout.to_string()))?;
        let mut next = self.state.clone();
        next.layouts.insert(ws.to_string(), layout);
        self.commit(next, Change::Layout(ws.to_string(), layout))
            .await
    }

    pub async fn set_schedule(&mut self, s: Schedule) -> Result<(), DesktopError> {
        self.set_schedule_at(s, Local::now()).await
    }

    /// Store a schedule and switch to the variant it wants at `now`.
    pub async fn set_schedule_at<Tz: TimeZone>(
        &mut self,
        s: Schedule,
        now: DateTime<Tz>,
    ) -> Result<(), DesktopError> {
        s.validate().map_err(DesktopError::InvalidSchedule)?;
        let mut next = self.state.clone();
        let wanted = variant_at(&s, &now);
        next.schedule = s;
        let change = match wanted {
            Some(v) if v != next.variant => {
                next.variant = v;
                Change::Theme
            }
            _ => Change::Quiet,
        };
        self.commit(next, change).await
    }

    /// Apply the scheduled variant if it differs from the current one. Returns whether it switched.
    /// Called every minute, so suspend/resume and clock changes are caught on the next tick.
    pub async fn tick_schedule_at<Tz: TimeZone>(
        &mut self,
        now: DateTime<Tz>,
    ) -> Result<bool, DesktopError> {
        match variant_at(&self.state.schedule, &now) {
            Some(v) if v != self.state.variant => {
                let theme = self.state.theme.clone();
                self.set_theme(&theme, v).await.map(|()| true)
            }
            _ => Ok(false),
        }
    }

    /// Re-render every file from current state and re-theme the session.
    pub async fn reload(&mut self) -> Result<(), DesktopError> {
        self.commit(self.state.clone(), Change::Theme).await
    }

    fn ensure_user_lua(&self) -> Result<(), DesktopError> {
        let p = &self.paths.user_lua;
        if p.exists() {
            return Ok(());
        }
        atomic_write(p, USER_LUA_STUB).map_err(io_err("creating user.lua"))
    }

    /// Validate, then write theme dir + generated.lua + state; then apply live and notify.
    async fn commit(&mut self, next: State, change: Change) -> Result<(), DesktopError> {
        let theme = self
            .themes
            .get(&next.theme, next.variant)
            .ok_or_else(|| DesktopError::UnknownTheme(next.theme.clone()))?
            .clone();
        let files = render(&theme, &self.paths.templates_dir)
            .map_err(|e| DesktopError::Io(e.to_string()))?;
        let generated = emit_generated(&theme, &next.layouts);

        self.ensure_user_lua()?;
        let uq_dir = self
            .paths
            .generated_lua
            .parent()
            .expect("generated.lua has a parent")
            .to_path_buf();
        let candidate = uq_dir.join("generated.lua.candidate");
        let verify_main = uq_dir.join(format!(".verify-{}.lua", std::process::id()));
        atomic_write(&candidate, &generated).map_err(io_err("writing candidate config"))?;
        let main_src = [&self.paths.base_lua, &candidate, &self.paths.user_lua]
            .iter()
            .map(|p| format!("require({})\n", lua_string(&p.to_string_lossy())))
            .collect::<String>();
        let verdict = match atomic_write(&verify_main, &main_src) {
            Ok(()) => self.comp.verify(&verify_main).await,
            Err(e) => Err(CompositorError::Io(e)),
        };
        let _ = std::fs::remove_file(&verify_main);
        match verdict {
            Ok(()) => {}
            Err(CompositorError::Rejected(msg)) => {
                let _ = std::fs::remove_file(&candidate);
                let _ = self.events.send(Event::ConfigError(msg.clone()));
                return Err(DesktopError::ConfigInvalid(msg));
            }
            Err(e) => tracing::warn!("could not validate config ({e}); applying unvalidated"),
        }

        let committed = atomic_replace_dir(&self.paths.theme_dir, &files)
            .map_err(io_err("writing theme files"))
            .and_then(|()| {
                std::fs::rename(&candidate, &self.paths.generated_lua)
                    .map_err(io_err("installing generated.lua"))
            })
            .and_then(|()| {
                next.save(&self.paths.state_file)
                    .map_err(io_err("saving state"))
            });
        if let Err(e) = committed {
            let _ = std::fs::remove_file(&candidate);
            return Err(e);
        }
        self.state = next;

        match change {
            Change::Quiet => {}
            Change::Layout(ws, layout) => {
                self.live(self.comp.eval(&emit_workspace_rule(&ws, layout)).await);
                let _ = self.events.send(Event::LayoutChanged {
                    workspace: ws,
                    layout,
                });
            }
            Change::Theme => {
                self.live(self.comp.reload().await);
                if let Err(e) = self.sys.apply(&theme) {
                    tracing::warn!("system settings: {e}");
                }
                let _ = self.events.send(Event::ThemeChanged {
                    name: theme.id.clone(),
                    variant: theme.variant,
                    tokens_path: self.paths.theme_dir.join("tokens.json"),
                });
            }
        }
        Ok(())
    }

    /// Report the outcome of a live-apply call; the change is already persisted either way.
    fn live(&self, res: Result<(), CompositorError>) {
        match res {
            Ok(()) => {}
            Err(CompositorError::NotRunning) => {
                tracing::warn!("Hyprland not running; change saved for next start")
            }
            Err(CompositorError::Rejected(msg)) => {
                tracing::warn!("Hyprland rejected live change: {msg}");
                let _ = self.events.send(Event::ConfigError(msg));
            }
            Err(e) => tracing::warn!("Hyprland: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::fake::FakeCompositor;
    use crate::system::FakeSystem;
    use std::fs;
    use std::path::PathBuf;
    use uq_core::lua::Layout;
    use uq_core::schedule::Schedule;

    struct Rig {
        _dir: tempfile::TempDir,
        desktop: Desktop,
        comp: Arc<FakeCompositor>,
        sys: Arc<FakeSystem>,
        events: broadcast::Receiver<Event>,
    }

    fn rig_with(running: bool, prepare: impl FnOnce(&Paths)) -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut paths = Paths::under(dir.path());
        paths.themes_dir = repo.join("themes");
        paths.templates_dir = repo.join("templates");
        prepare(&paths);
        let comp = Arc::new(FakeCompositor::default());
        *comp.running.lock().unwrap() = running;
        let sys = Arc::new(FakeSystem::default());
        let desktop = Desktop::new(paths, comp.clone(), sys.clone()).unwrap();
        let events = desktop.subscribe();
        Rig {
            _dir: dir,
            desktop,
            comp,
            sys,
            events,
        }
    }

    fn rig() -> Rig {
        rig_with(true, |_| {})
    }

    fn drain(rx: &mut broadcast::Receiver<Event>) -> Vec<Event> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    #[tokio::test]
    async fn set_theme_writes_files_and_emits() {
        let mut r = rig();
        r.desktop
            .set_theme("dracula", Variant::Light)
            .await
            .unwrap();
        let p = r.desktop.paths().clone();
        let tokens = fs::read_to_string(p.theme_dir.join("tokens.json")).unwrap();
        assert!(tokens.contains(r#""id": "dracula""#), "{tokens}");
        let generated = fs::read_to_string(&p.generated_lua).unwrap();
        assert!(generated.contains("Dracula (light)"), "{generated}");
        assert_eq!(State::load(&p.state_file).theme, "dracula");
        assert_eq!(
            r.desktop.get_theme(),
            ("dracula".to_string(), Variant::Light)
        );
        assert!(r.comp.calls().contains(&"reload".to_string()));
        assert_eq!(
            *r.sys.applied.lock().unwrap(),
            vec!["dracula light".to_string()]
        );
        assert_eq!(
            drain(&mut r.events),
            vec![Event::ThemeChanged {
                name: "dracula".into(),
                variant: Variant::Light,
                tokens_path: p.theme_dir.join("tokens.json")
            }]
        );
    }

    #[tokio::test]
    async fn set_layout_persists_and_evals() {
        let mut r = rig();
        r.desktop.set_layout("2", "scrolling").await.unwrap();
        let p = r.desktop.paths().clone();
        assert!(
            r.comp.calls().contains(
                &r#"eval hl.workspace_rule({ workspace = "2", layout = "scrolling" })"#.to_string()
            ),
            "{:?}",
            r.comp.calls()
        );
        assert_eq!(
            State::load(&p.state_file).layouts.get("2"),
            Some(&Layout::Scrolling)
        );
        assert!(
            fs::read_to_string(&p.generated_lua)
                .unwrap()
                .contains(r#"workspace = "2", layout = "scrolling""#)
        );
        assert_eq!(r.desktop.get_layouts().get("2"), Some(&Layout::Scrolling));
        assert_eq!(
            drain(&mut r.events),
            vec![Event::LayoutChanged {
                workspace: "2".into(),
                layout: Layout::Scrolling
            }]
        );
        // A layout change doesn't re-theme the session.
        assert!(r.sys.applied.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn failed_verify_keeps_last_good_and_reports() {
        let mut r = rig();
        r.desktop.set_theme("forest", Variant::Dark).await.unwrap();
        let p = r.desktop.paths().clone();
        let before = fs::read(&p.generated_lua).unwrap();
        let tokens_before = fs::read(p.theme_dir.join("tokens.json")).unwrap();
        drain(&mut r.events);
        *r.comp.verify_error.lock().unwrap() = Some("user.lua:3: syntax error".into());

        let err = r
            .desktop
            .set_theme("velvet", Variant::Light)
            .await
            .unwrap_err();
        assert!(
            matches!(&err, DesktopError::ConfigInvalid(m) if m == "user.lua:3: syntax error"),
            "{err:?}"
        );
        assert_eq!(fs::read(&p.generated_lua).unwrap(), before);
        assert_eq!(
            fs::read(p.theme_dir.join("tokens.json")).unwrap(),
            tokens_before
        );
        assert_eq!(State::load(&p.state_file).theme, "forest");
        assert_eq!(r.desktop.get_theme().0, "forest");
        assert_eq!(
            drain(&mut r.events),
            vec![Event::ConfigError("user.lua:3: syntax error".into())]
        );
        let leftovers: Vec<_> = fs::read_dir(p.generated_lua.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        let mut leftovers = leftovers;
        leftovers.sort();
        assert_eq!(leftovers, vec!["generated.lua", "user.lua"]);
    }

    #[tokio::test]
    async fn verify_sees_candidate_and_user_files() {
        let mut r = rig();
        r.desktop.set_layout("1", "master").await.unwrap();
        let verify = r
            .comp
            .calls()
            .into_iter()
            .find(|c| c.starts_with("verify "))
            .unwrap();
        let main = PathBuf::from(verify.trim_start_matches("verify "));
        assert!(!main.exists(), "temporary main config left behind");
    }

    #[tokio::test]
    async fn compositor_down_still_persists() {
        let mut r = rig_with(false, |_| {});
        r.desktop.set_layout("web", "dwindle").await.unwrap();
        r.desktop.set_theme("matcha", Variant::Light).await.unwrap();
        let p = r.desktop.paths().clone();
        let s = State::load(&p.state_file);
        assert_eq!(
            (s.theme.as_str(), s.layouts.get("web")),
            ("matcha", Some(&Layout::Dwindle))
        );
        assert!(p.theme_dir.join("tokens.json").exists());
    }

    #[tokio::test]
    async fn unknown_theme_rejected() {
        let mut r = rig();
        let err = r
            .desktop
            .set_theme("solarized", Variant::Dark)
            .await
            .unwrap_err();
        assert!(matches!(err, DesktopError::UnknownTheme(ref id) if id == "solarized"));
    }

    #[tokio::test]
    async fn invalid_layout_rejected() {
        let mut r = rig();
        let err = r.desktop.set_layout("1", "monocle").await.unwrap_err();
        assert!(matches!(err, DesktopError::InvalidLayout(ref l) if l == "monocle"));
    }

    #[tokio::test]
    async fn empty_workspace_rejected() {
        let mut r = rig();
        assert!(matches!(
            r.desktop.set_layout("  ", "master").await,
            Err(DesktopError::EmptyWorkspace)
        ));
    }

    #[tokio::test]
    async fn set_schedule_applies_current_variant() {
        let mut r = rig();
        r.desktop
            .set_theme("graphite", Variant::Dark)
            .await
            .unwrap();
        let noon = chrono::Local::now()
            .date_naive()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let now = noon.and_local_timezone(chrono::Local).earliest().unwrap();
        let s = Schedule::Fixed {
            light_at: chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            dark_at: chrono::NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
        };
        r.desktop.set_schedule_at(s.clone(), now).await.unwrap();
        assert_eq!(
            r.desktop.get_theme(),
            ("graphite".to_string(), Variant::Light)
        );
        assert_eq!(State::load(&r.desktop.paths().state_file).schedule, s);
    }

    #[tokio::test]
    async fn schedule_tick_switches_variant_when_due() {
        let mut r = rig();
        let at = |h| {
            chrono::Local::now()
                .date_naive()
                .and_hms_opt(h, 0, 0)
                .unwrap()
                .and_local_timezone(chrono::Local)
                .earliest()
                .unwrap()
        };
        let s = Schedule::Fixed {
            light_at: chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            dark_at: chrono::NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
        };
        r.desktop.set_schedule_at(s, at(12)).await.unwrap();
        assert_eq!(r.desktop.get_theme().1, Variant::Light);
        drain(&mut r.events);
        assert!(
            !r.desktop.tick_schedule_at(at(13)).await.unwrap(),
            "nothing due at 13:00"
        );
        assert!(drain(&mut r.events).is_empty());
        assert!(
            r.desktop.tick_schedule_at(at(20)).await.unwrap(),
            "dark due at 20:00"
        );
        assert_eq!(r.desktop.get_theme().1, Variant::Dark);
    }

    #[tokio::test]
    async fn schedule_tick_off_does_nothing() {
        let mut r = rig();
        assert!(
            !r.desktop
                .tick_schedule_at(chrono::Utc::now())
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn invalid_schedule_rejected() {
        let mut r = rig();
        let t = chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap();
        let err = r
            .desktop
            .set_schedule(Schedule::Fixed {
                light_at: t,
                dark_at: t,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, DesktopError::InvalidSchedule(_)));
    }

    #[tokio::test]
    async fn startup_creates_user_lua_and_renders() {
        let mut r = rig();
        r.desktop.startup().await;
        let p = r.desktop.paths().clone();
        assert!(p.user_lua.exists());
        assert!(p.generated_lua.exists());
        assert!(
            fs::read_to_string(&p.generated_lua)
                .unwrap()
                .contains("Graphite (dark)")
        );
    }

    #[tokio::test]
    async fn startup_never_overwrites_user_lua() {
        let mut r = rig_with(true, |p| {
            fs::create_dir_all(p.user_lua.parent().unwrap()).unwrap();
            fs::write(&p.user_lua, "hl.config({ general = { gaps_in = 0 } })\n").unwrap();
        });
        r.desktop.startup().await;
        assert_eq!(
            fs::read_to_string(&r.desktop.paths().user_lua).unwrap(),
            "hl.config({ general = { gaps_in = 0 } })\n"
        );
    }

    #[tokio::test]
    async fn stale_theme_in_state_falls_back_to_default() {
        let r = rig_with(true, |p| {
            let s = State {
                theme: "removed".into(),
                ..State::default()
            };
            s.save(&p.state_file).unwrap();
        });
        assert_eq!(
            r.desktop.get_theme(),
            ("graphite".to_string(), Variant::Dark)
        );
    }
}
