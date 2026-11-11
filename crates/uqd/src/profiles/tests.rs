use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::power::PowerError;
use crate::power::fake::FakePower;
use crate::system::FakeSystem;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use uq_core::history::{AutoReason, Origin, PointKind};
use uq_core::paths::Paths;
use uq_core::profile::{Profile, power_notice, read_leaf, save_active};
use uq_core::schedule::Schedule;
use uq_core::theme::Variant;

const GAMING: &str =
    "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[look]\ngaps_in = 0\nblur = false\n";
const WORK: &str =
    "format = 1\nid = \"work\"\nname = \"Work\"\n[look]\ngaps_in = 8\nrounding = 2\n";
const TURBO: &str = "format = 1\nid = \"turbo\"\nname = \"Turbo\"\n[look]\ngaps_in = 0\n[power]\nprofile = \"performance\"\n";
const NIGHT: &str =
    "format = 1\nid = \"night\"\nname = \"Night\"\n[appearance]\nvariant = \"dark\"\n";

struct Rig {
    dir: tempfile::TempDir,
    desktop: Desktop,
    comp: Arc<FakeCompositor>,
    power: Arc<FakePower>,
    events: broadcast::Receiver<Event>,
}

fn paths(dir: &std::path::Path) -> Paths {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir);
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    paths
}

/// A desktop on `dir`'s paths, as uqd builds it at start.
fn desktop_on(dir: &std::path::Path, comp: Arc<FakeCompositor>, power: Arc<FakePower>) -> Desktop {
    Desktop::new(paths(dir), comp, Arc::new(FakeSystem::default()))
        .unwrap()
        .with_power(power)
}

async fn rig_with(power: FakePower) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let comp = Arc::new(FakeCompositor::running());
    let power = Arc::new(power);
    let mut desktop = desktop_on(dir.path(), comp.clone(), power.clone());
    desktop.startup().await;
    // Start every test from gaps 5 and blur on, as a person's own settings.
    desktop
        .set_section("look", r#"{"gaps_in":5,"blur":true}"#, Origin::Settings)
        .await
        .unwrap();
    let events = desktop.subscribe();
    Rig {
        dir,
        desktop,
        comp,
        power,
        events,
    }
}

async fn rig() -> Rig {
    rig_with(FakePower::new("balanced")).await
}

fn drain(rx: &mut broadcast::Receiver<Event>) -> Vec<Event> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

fn profile_events(rx: &mut broadcast::Receiver<Event>) -> Vec<String> {
    drain(rx)
        .into_iter()
        .filter_map(|e| match e {
            Event::ProfileChanged(id) => Some(id),
            _ => None,
        })
        .collect()
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn look(r: &Rig, key: &str) -> Option<Value> {
    read_leaf(r.desktop.state(), "look", &s(&[key]))
}

/// Change one `look` key by hand, keeping the rest of the section.
async fn set_look(r: &mut Rig, key: &str, v: Value) {
    let mut section: Value = serde_json::from_str(&r.desktop.get_section("look").unwrap()).unwrap();
    section[key] = v;
    r.desktop
        .set_section("look", &section.to_string(), Origin::Settings)
        .await
        .unwrap();
}

async fn add(r: &mut Rig, toml: &str) {
    r.desktop.set_profile(toml, false).unwrap();
}

fn journal_len(r: &Rig) -> usize {
    r.desktop.history(1000).len()
}

fn read(path: &std::path::Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

#[tokio::test]
async fn apply_sets_keys_with_profile_origin() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    let before = journal_len(&r);
    let report = r.desktop.apply_profile("gaming").await.unwrap();
    assert_eq!(report.active, "gaming");
    assert!(
        report.kept.is_empty() && report.notices.is_empty(),
        "{report:?}"
    );
    assert_eq!(look(&r, "gaps_in"), Some(json!(0)));
    assert_eq!(look(&r, "blur"), Some(json!(false)));
    let entries = r.desktop.history(1000);
    let new = &entries[..entries.len() - before];
    assert!(!new.is_empty());
    assert!(new.iter().all(|e| e.origin == "profile:gaming"), "{new:?}");
    assert_eq!(r.desktop.active_profile().unwrap().id, "gaming");
}

#[tokio::test]
async fn apply_saves_before_profile_point() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    let had: Vec<_> = r
        .desktop
        .restore_points()
        .into_iter()
        .map(|p| p.id)
        .collect();
    r.desktop.apply_profile("gaming").await.unwrap();
    // Points made in the same second sort by a random suffix, so look for the new one.
    let new: Vec<_> = r
        .desktop
        .restore_points()
        .into_iter()
        .filter(|p| !had.contains(&p.id))
        .collect();
    assert_eq!(new.len(), 1, "{new:?}");
    assert_eq!(new[0].kind, PointKind::Auto(AutoReason::BeforeProfile));
    assert_eq!(new[0].name, "Before the Gaming profile");
    // The point holds the settings from before the profile.
    let saved = r.desktop.restore.load(&new[0].id).unwrap();
    assert_eq!(read_leaf(&saved, "look", &s(&["gaps_in"])), Some(json!(5)));
}

#[tokio::test]
async fn off_restores_untouched_and_reports_hand_changed() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    r.desktop.apply_profile("gaming").await.unwrap();
    set_look(&mut r, "gaps_in", json!(3)).await;
    let before = journal_len(&r);
    let report = r.desktop.profile_off().await.unwrap();
    assert_eq!(look(&r, "blur"), Some(json!(true)));
    assert_eq!(look(&r, "gaps_in"), Some(json!(3)));
    let labels: Vec<_> = report.kept.iter().map(|k| k.label.as_str()).collect();
    assert_eq!(labels, ["Gaps between windows"]);
    assert_eq!(report.active, "");
    assert!(r.desktop.active_profile().is_none());
    assert!(!r.desktop.paths().profile_active.exists());
    let entries = r.desktop.history(1000);
    let new = &entries[..entries.len() - before];
    assert!(!new.is_empty());
    assert!(new.iter().all(|e| e.origin == "profile:gaming"), "{new:?}");
}

#[tokio::test]
async fn off_removes_a_display_entry_the_profile_created() {
    let mut r = rig().await;
    r.desktop
        .set_section(
            "wallpaper",
            r#"{"outputs":{"*":{"image":"/w/a.png"}}}"#,
            Origin::Settings,
        )
        .await
        .unwrap();
    let before = r.desktop.get_section("wallpaper").unwrap();
    add(
        &mut r,
        "format = 1\nid = \"hdmi\"\nname = \"HDMI\"\n[wallpaper.outputs.HDMI-A-1]\nimage = \"/w/b.png\"\n",
    )
    .await;
    r.desktop.apply_profile("hdmi").await.unwrap();
    assert_ne!(r.desktop.get_section("wallpaper").unwrap(), before);
    let report = r.desktop.profile_off().await.unwrap();
    assert!(report.kept.is_empty(), "{report:?}");
    assert_eq!(r.desktop.get_section("wallpaper").unwrap(), before);
}

#[tokio::test]
async fn switching_profiles_returns_untouched_keys_then_applies_next() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    add(&mut r, WORK).await;
    r.desktop.apply_profile("gaming").await.unwrap();
    set_look(&mut r, "gaps_in", json!(3)).await;
    let before = journal_len(&r);
    let report = r.desktop.apply_profile("work").await.unwrap();
    assert_eq!(report.active, "work");
    let labels: Vec<_> = report.kept.iter().map(|k| k.label.as_str()).collect();
    assert_eq!(labels, ["Gaps between windows"]);
    // Gaming's untouched key is back, Work's keys are on.
    assert_eq!(look(&r, "blur"), Some(json!(true)));
    assert_eq!(look(&r, "gaps_in"), Some(json!(8)));
    assert_eq!(look(&r, "rounding"), Some(json!(2)));
    // One write, journalled as Work.
    let entries = r.desktop.history(1000);
    let new = &entries[..entries.len() - before];
    assert_eq!(new.len(), 1, "{new:?}");
    assert_eq!(new[0].origin, "profile:work");
    // Work remembers the hand value, so Back to normal goes to it.
    r.desktop.profile_off().await.unwrap();
    assert_eq!(look(&r, "gaps_in"), Some(json!(3)));
    assert_eq!(look(&r, "rounding"), None);
    assert_eq!(look(&r, "blur"), Some(json!(true)));
}

#[tokio::test]
async fn active_profile_survives_restart() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    r.desktop.apply_profile("gaming").await.unwrap();
    let mut again = desktop_on(r.dir.path(), r.comp.clone(), r.power.clone());
    assert_eq!(again.active_profile().unwrap().id, "gaming");
    again.profile_off().await.unwrap();
    let st = again.state();
    assert_eq!(read_leaf(st, "look", &s(&["gaps_in"])), Some(json!(5)));
    assert_eq!(read_leaf(st, "look", &s(&["blur"])), Some(json!(true)));
}

#[tokio::test]
async fn active_file_written_but_state_not_still_turns_off_cleanly() {
    let r = rig().await;
    // uqd stopped after writing the active file and before the commit.
    let p = Profile::parse(GAMING).unwrap();
    let (_, memo) = uq_core::profile::apply(r.desktop.state(), &p, &Default::default(), 1).unwrap();
    save_active(&r.desktop.paths().profile_active, &memo).unwrap();
    let mut again = desktop_on(r.dir.path(), r.comp.clone(), r.power.clone());
    let state_file = read(&again.paths().state_file);
    let entries = again.history(1000).len();
    let report = again.profile_off().await.unwrap();
    assert_eq!(report, ProfileReport::default());
    assert_eq!(read(&again.paths().state_file), state_file);
    assert_eq!(again.history(1000).len(), entries);
    assert!(again.active_profile().is_none());
    assert!(!again.paths().profile_active.exists());
}

#[tokio::test]
async fn failed_verify_changes_nothing_and_keeps_previous_profile() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    add(&mut r, WORK).await;
    r.desktop.apply_profile("gaming").await.unwrap();
    let state = r.desktop.state().clone();
    let active_file = read(&r.desktop.paths().profile_active);
    let points = r.desktop.restore_points().len();
    let entries = journal_len(&r);
    drain(&mut r.events);
    *r.comp.verify_error.lock().unwrap() = Some("user.lua:3: bad".into());
    let err = r.desktop.apply_profile("work").await.unwrap_err();
    assert!(matches!(err, DesktopError::ConfigInvalid(_)), "{err:?}");
    assert_eq!(*r.desktop.state(), state);
    assert_eq!(read(&r.desktop.paths().profile_active), active_file);
    assert_eq!(r.desktop.active_profile().unwrap().id, "gaming");
    assert_eq!(r.desktop.restore_points().len(), points);
    assert_eq!(journal_len(&r), entries);
    assert!(profile_events(&mut r.events).is_empty());
    // With no profile on before, a failed apply leaves no active file.
    *r.comp.verify_error.lock().unwrap() = None;
    r.desktop.profile_off().await.unwrap();
    *r.comp.verify_error.lock().unwrap() = Some("user.lua:3: bad".into());
    r.desktop.apply_profile("work").await.unwrap_err();
    assert!(!r.desktop.paths().profile_active.exists());
    assert!(r.desktop.active_profile().is_none());
}

#[tokio::test]
async fn apply_with_uninstalled_theme_changes_nothing() {
    let mut r = rig().await;
    // Saved while the theme was there; set_profile would refuse it now.
    let text = "format = 1\nid = \"gone\"\nname = \"Gone\"\n[appearance]\ntheme = \"nosuch\"\n[look]\ngaps_in = 0\n[power]\nprofile = \"performance\"\n";
    let dir = &r.desktop.paths().profiles_dir;
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("gone.toml"), text).unwrap();
    let state = r.desktop.state().clone();
    let points = r.desktop.restore_points().len();
    let err = r.desktop.apply_profile("gone").await.unwrap_err();
    assert_eq!(err, DesktopError::UnknownTheme("nosuch".into()));
    assert_eq!(*r.desktop.state(), state);
    assert_eq!(r.desktop.restore_points().len(), points);
    assert!(!r.desktop.paths().profile_active.exists());
    assert!(r.desktop.active_profile().is_none());
    assert!(r.power.sets.lock().unwrap().is_empty());
    assert!(profile_events(&mut r.events).is_empty());
}

#[tokio::test]
async fn off_with_the_old_theme_uninstalled_uses_the_default_theme() {
    let mut r = rig().await;
    add(
        &mut r,
        "format = 1\nid = \"red\"\nname = \"Red\"\n[appearance]\ntheme = \"rosewood\"\n[look]\ngaps_in = 0\n",
    )
    .await;
    r.desktop.apply_profile("red").await.unwrap();
    // The theme in use before the profile has been uninstalled since.
    let mut memo = r.desktop.active_profile().unwrap().clone();
    let theme = memo.keys.iter_mut().find(|m| m.path == ["theme"]).unwrap();
    theme.before = Some(json!("nosuch"));
    save_active(&r.desktop.paths().profile_active, &memo).unwrap();
    let mut again = desktop_on(r.dir.path(), r.comp.clone(), r.power.clone());
    let report = again.profile_off().await.unwrap();
    assert_eq!(again.state().theme, uq_core::state::State::default().theme);
    assert_eq!(
        read_leaf(again.state(), "look", &s(&["gaps_in"])),
        Some(json!(5))
    );
    assert!(again.active_profile().is_none());
    assert_eq!(report.notices, [THEME_GONE], "{report:?}");
}

#[tokio::test]
async fn power_missing_is_a_notice_not_a_failure() {
    let mut r = rig_with(FakePower::missing()).await;
    add(&mut r, TURBO).await;
    let report = r.desktop.apply_profile("turbo").await.unwrap();
    assert_eq!(
        report.notices,
        [power_notice(&PowerError::Missing.to_string())]
    );
    assert_eq!(look(&r, "gaps_in"), Some(json!(0)));
    assert_eq!(r.desktop.active_profile().unwrap().id, "turbo");
}

#[tokio::test]
async fn power_refused_is_a_notice() {
    let power = FakePower::new("balanced");
    power.refuse.store(true, Ordering::SeqCst);
    let mut r = rig_with(power).await;
    add(&mut r, TURBO).await;
    let report = r.desktop.apply_profile("turbo").await.unwrap();
    assert_eq!(
        report.notices,
        [power_notice(
            &PowerError::Refused(String::new()).to_string()
        )]
    );
    assert_eq!(look(&r, "gaps_in"), Some(json!(0)));
    assert_eq!(*r.power.mode.lock().unwrap(), Some("balanced".into()));
}

#[tokio::test]
async fn power_timeout_is_a_notice() {
    let power = FakePower::new("balanced");
    // Far past the 50 ms timeout, so a loaded machine can't blur the two.
    *power.delay.lock().unwrap() = Duration::from_secs(30);
    let mut r = rig_with(power).await;
    r.desktop.power_timeout = Duration::from_millis(50);
    add(&mut r, TURBO).await;
    let t = Instant::now();
    let report = r.desktop.apply_profile("turbo").await.unwrap();
    assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
    assert_eq!(
        report.notices,
        [power_notice(&PowerError::Timeout.to_string())]
    );
    assert_eq!(look(&r, "gaps_in"), Some(json!(0)));
}

#[tokio::test]
async fn power_set_that_times_out_is_still_put_back_if_it_lands() {
    let mut r = rig().await;
    add(&mut r, TURBO).await;
    // The read answers in time; the set doesn't, but lands later.
    r.desktop.power_timeout = Duration::from_millis(50);
    let slow = Arc::new(SlowSet(r.power.clone()));
    r.desktop.power = slow;
    let report = r.desktop.apply_profile("turbo").await.unwrap();
    assert_eq!(
        report.notices,
        [power_notice(&PowerError::Timeout.to_string())]
    );
    *r.power.mode.lock().unwrap() = Some("performance".into());
    r.desktop.power = r.power.clone();
    let report = r.desktop.profile_off().await.unwrap();
    assert!(report.kept.is_empty(), "{report:?}");
    assert_eq!(*r.power.mode.lock().unwrap(), Some("balanced".into()));
}

/// Reads at once, never finishes a set.
struct SlowSet(Arc<FakePower>);

#[async_trait::async_trait]
impl crate::power::PowerProfiles for SlowSet {
    async fn active(&self) -> Result<String, PowerError> {
        self.0.active().await
    }
    async fn set_active(&self, _mode: &str) -> Result<(), PowerError> {
        std::future::pending().await
    }
}

#[tokio::test]
async fn off_puts_power_back_only_if_unchanged() {
    let mut r = rig().await;
    add(&mut r, TURBO).await;
    let mode = |r: &Rig| r.power.mode.lock().unwrap().clone().unwrap();

    r.desktop.apply_profile("turbo").await.unwrap();
    assert_eq!(mode(&r), "performance");
    let report = r.desktop.profile_off().await.unwrap();
    assert_eq!(mode(&r), "balanced");
    assert!(
        report.kept.is_empty() && report.notices.is_empty(),
        "{report:?}"
    );

    // Changed by hand while the profile was on: kept and named.
    r.desktop.apply_profile("turbo").await.unwrap();
    *r.power.mode.lock().unwrap() = Some("power-saver".into());
    let report = r.desktop.profile_off().await.unwrap();
    assert_eq!(mode(&r), "power-saver");
    let labels: Vec<_> = report.kept.iter().map(|k| k.label.as_str()).collect();
    assert_eq!(labels, ["Power mode"]);

    // Already back where it was: nothing set, nothing reported.
    *r.power.mode.lock().unwrap() = Some("balanced".into());
    r.desktop.apply_profile("turbo").await.unwrap();
    *r.power.mode.lock().unwrap() = Some("balanced".into());
    let sets = r.power.sets.lock().unwrap().len();
    let report = r.desktop.profile_off().await.unwrap();
    assert!(
        report.kept.is_empty() && report.notices.is_empty(),
        "{report:?}"
    );
    assert_eq!(r.power.sets.lock().unwrap().len(), sets);
}

#[tokio::test]
async fn switching_keeps_the_first_power_before() {
    let mut r = rig().await;
    add(&mut r, TURBO).await;
    add(
        &mut r,
        "format = 1\nid = \"quiet\"\nname = \"Quiet\"\n[look]\ngaps_in = 2\n[power]\nprofile = \"power-saver\"\n",
    )
    .await;
    r.desktop.apply_profile("turbo").await.unwrap();
    r.desktop.apply_profile("quiet").await.unwrap();
    assert_eq!(*r.power.mode.lock().unwrap(), Some("power-saver".into()));
    r.desktop.profile_off().await.unwrap();
    assert_eq!(*r.power.mode.lock().unwrap(), Some("balanced".into()));
    // A profile without a power mode puts the first one's back.
    r.desktop.apply_profile("turbo").await.unwrap();
    add(&mut r, GAMING).await;
    r.desktop.apply_profile("gaming").await.unwrap();
    assert_eq!(*r.power.mode.lock().unwrap(), Some("balanced".into()));
}

#[tokio::test]
async fn restore_ends_the_active_profile() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    let id = r.desktop.save_restore_point("mine").unwrap();
    r.desktop.apply_profile("gaming").await.unwrap();
    drain(&mut r.events);
    r.desktop.restore(&id).await.unwrap();
    assert!(r.desktop.active_profile().is_none());
    assert!(!r.desktop.paths().profile_active.exists());
    assert_eq!(profile_events(&mut r.events), [""]);
    assert_eq!(look(&r, "gaps_in"), Some(json!(5)));
}

#[tokio::test]
async fn restore_puts_the_profiles_power_mode_back() {
    let mut r = rig().await;
    add(&mut r, TURBO).await;
    let id = r.desktop.save_restore_point("mine").unwrap();
    r.desktop.apply_profile("turbo").await.unwrap();
    assert_eq!(*r.power.mode.lock().unwrap(), Some("performance".into()));
    let notices = r.desktop.restore(&id).await.unwrap();
    assert!(notices.is_empty(), "{notices:?}");
    assert_eq!(*r.power.mode.lock().unwrap(), Some("balanced".into()));
    assert!(r.desktop.active_profile().is_none());

    // Changed by hand while the profile was on: left alone.
    r.desktop.apply_profile("turbo").await.unwrap();
    *r.power.mode.lock().unwrap() = Some("power-saver".into());
    r.desktop.restore(&id).await.unwrap();
    assert_eq!(*r.power.mode.lock().unwrap(), Some("power-saver".into()));
}

#[tokio::test]
async fn restore_with_power_refused_still_restores_with_a_notice() {
    let mut r = rig().await;
    add(&mut r, TURBO).await;
    let id = r.desktop.save_restore_point("mine").unwrap();
    r.desktop.apply_profile("turbo").await.unwrap();
    r.power.refuse.store(true, Ordering::SeqCst);
    let notices = r.desktop.restore(&id).await.unwrap();
    assert_eq!(
        notices,
        [power_notice(
            &PowerError::Refused(String::new()).to_string()
        )]
    );
    assert_eq!(look(&r, "gaps_in"), Some(json!(5)));
    assert!(r.desktop.active_profile().is_none());
}

#[tokio::test]
async fn explain_reports_profile_source() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    r.desktop.apply_profile("gaming").await.unwrap();
    let v = r.desktop.explain("look").await.unwrap();
    assert_eq!(v["gaps_in"]["source"], "profile", "{v}");
    assert_eq!(v["gaps_in"]["detail"]["name"], "Gaming");
    set_look(&mut r, "gaps_in", json!(3)).await;
    let v = r.desktop.explain("look").await.unwrap();
    assert_eq!(v["gaps_in"]["source"], "uq", "{v}");
    assert_eq!(v["blur"]["source"], "profile", "{v}");
}

#[tokio::test]
async fn set_profile_conflict_then_replace() {
    let mut r = rig().await;
    assert_eq!(r.desktop.set_profile(GAMING, false).unwrap(), "gaming");
    let renamed = GAMING.replace("name = \"Gaming\"", "name = \"Games\"");
    assert_eq!(
        r.desktop.set_profile(&renamed, false).unwrap_err(),
        DesktopError::Conflict("A profile called “Gaming” already exists. Replace it?".into())
    );
    assert_eq!(r.desktop.set_profile(&renamed, true).unwrap(), "gaming");
    assert!(
        r.desktop
            .get_profile("gaming")
            .unwrap()
            .contains("name = \"Games\"")
    );
    let list = r.desktop.list_profiles();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Games");
    assert_eq!(list[0].keys, 2);
    assert!(!list[0].active);
}

#[tokio::test]
async fn set_profile_with_unknown_theme_is_refused() {
    let mut r = rig().await;
    let text = "format = 1\nid = \"gone\"\nname = \"Gone\"\n[appearance]\ntheme = \"nosuch\"\n";
    assert_eq!(
        r.desktop.set_profile(text, false).unwrap_err(),
        DesktopError::UnknownTheme("nosuch".into())
    );
    assert!(r.desktop.list_profiles().is_empty());
    let bad = "format = 1\nid = \"x\"\nname = \"X\"\n[look]\ngaps_in = 9999\n";
    assert!(matches!(
        r.desktop.set_profile(bad, false),
        Err(DesktopError::InvalidSetting(_))
    ));
    assert_eq!(
        r.desktop.get_profile("nope").unwrap_err(),
        DesktopError::InvalidArgument("no profile \"nope\"".into())
    );
}

#[tokio::test]
async fn delete_active_profile_is_refused() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    r.desktop.apply_profile("gaming").await.unwrap();
    assert_eq!(
        r.desktop.delete_profile("gaming").unwrap_err(),
        DesktopError::InvalidArgument("Turn this profile off before deleting it.".into())
    );
    r.desktop.profile_off().await.unwrap();
    r.desktop.delete_profile("gaming").unwrap();
    assert!(r.desktop.list_profiles().is_empty());
}

#[tokio::test]
async fn fifty_first_profile_is_refused() {
    let mut r = rig().await;
    let text =
        |i: usize| format!("format = 1\nid = \"p{i}\"\nname = \"P{i}\"\n[look]\ngaps_in = 1\n");
    for i in 0..50 {
        r.desktop.set_profile(&text(i), false).unwrap();
    }
    assert_eq!(
        r.desktop.set_profile(&text(50), false).unwrap_err(),
        DesktopError::InvalidArgument("You can keep up to 50 profiles.".into())
    );
    // Replacing one of the fifty is still fine.
    r.desktop.set_profile(&text(3), true).unwrap();
    assert_eq!(r.desktop.list_profiles().len(), 50);
}

#[tokio::test]
async fn profile_changed_event_sent() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    assert_eq!(profile_events(&mut r.events), [""]);
    r.desktop.apply_profile("gaming").await.unwrap();
    assert_eq!(profile_events(&mut r.events), ["gaming"]);
    add(&mut r, WORK).await;
    assert_eq!(profile_events(&mut r.events), ["gaming"]);
    r.desktop.profile_off().await.unwrap();
    assert_eq!(profile_events(&mut r.events), [""]);
    r.desktop.delete_profile("work").unwrap();
    assert_eq!(profile_events(&mut r.events), [""]);
    let list = r.desktop.list_profiles();
    assert_eq!(list.len(), 1);
}

#[tokio::test]
async fn schedule_holds_the_variant_a_profile_sets() {
    let mut r = rig().await;
    add(&mut r, NIGHT).await;
    let day = chrono::Local::now().date_naive();
    let tomorrow = day.succ_opt().unwrap();
    let at = |d: chrono::NaiveDate, h, m| {
        d.and_hms_opt(h, m, 0)
            .unwrap()
            .and_local_timezone(chrono::Local)
            .earliest()
            .unwrap()
    };
    let sched = Schedule::Fixed {
        light_at: chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
        dark_at: chrono::NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
    };
    r.desktop
        .set_schedule_at(sched.clone(), at(day, 20, 0), Origin::Settings)
        .await
        .unwrap();
    assert_eq!(r.desktop.get_theme().1, Variant::Dark);
    r.desktop.apply_profile("night").await.unwrap();
    // Sunrise while the profile holds dark: still dark.
    assert!(
        !r.desktop
            .tick_schedule_at(at(tomorrow, 7, 1))
            .await
            .unwrap()
    );
    assert_eq!(r.desktop.get_theme().1, Variant::Dark);
    // Storing the schedule again doesn't flip it either.
    r.desktop
        .set_schedule_at(sched, at(tomorrow, 8, 0), Origin::Settings)
        .await
        .unwrap();
    assert_eq!(r.desktop.get_theme().1, Variant::Dark);
    r.desktop.profile_off().await.unwrap();
    assert_eq!(r.desktop.get_theme().1, Variant::Dark);
    // The first tick after Back to normal applies the scheduled variant.
    assert!(
        r.desktop
            .tick_schedule_at(at(tomorrow, 8, 1))
            .await
            .unwrap()
    );
    assert_eq!(r.desktop.get_theme().1, Variant::Light);
}

/// A window rule "Pavucontrol floats", on; returns its id.
async fn add_rule(r: &mut Rig) -> String {
    r.desktop
        .set_section(
            "rules",
            r#"{"window":[{"name":"Pavucontrol floats","enabled":true,
                "match":{"class":"^pavucontrol$"},"effects":{"float":true}}]}"#,
            Origin::Settings,
        )
        .await
        .unwrap();
    r.desktop.state().rules.window[0].id.clone()
}

fn rules_profile(body: &str) -> String {
    format!("format = 1\nid = \"quiet\"\nname = \"Quiet\"\n[rules]\n{body}")
}

#[tokio::test]
async fn profile_turns_a_rule_off_and_back() {
    let mut r = rig().await;
    let id = add_rule(&mut r).await;
    add(&mut r, &rules_profile(&format!("\"{id}\" = false\n"))).await;
    let before = journal_len(&r);
    let report = r.desktop.apply_profile("quiet").await.unwrap();
    assert!(report.notices.is_empty(), "{report:?}");
    assert!(!r.desktop.state().rules.window[0].enabled);
    let entries = r.desktop.history(1000);
    let new = &entries[..entries.len() - before];
    assert_eq!(new.len(), 1, "{new:?}");
    assert_eq!(new[0].domain, "rules");
    assert_eq!(new[0].origin, "profile:quiet");
    assert_eq!(
        new[0].summary,
        "Window rule “Pavucontrol floats” turned off"
    );
    let g = std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap();
    assert!(
        g.contains(&format!("name = \"uq:{id}\", enabled = false")),
        "{g}"
    );
    assert!(r.comp.calls().iter().any(|c| c.starts_with("verify")));
    let report = r.desktop.profile_off().await.unwrap();
    assert!(report.kept.is_empty(), "{report:?}");
    assert!(r.desktop.state().rules.window[0].enabled);
    let g = std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap();
    assert!(
        g.contains(&format!("name = \"uq:{id}\", enabled = true")),
        "{g}"
    );
}

#[tokio::test]
async fn unknown_rule_id_is_a_notice() {
    let mut r = rig().await;
    let id = add_rule(&mut r).await;
    let other = if id == "r-ffff" { "r-fffe" } else { "r-ffff" };
    add(
        &mut r,
        &rules_profile(&format!("\"{id}\" = false\n\"{other}\" = true\n")),
    )
    .await;
    let report = r.desktop.apply_profile("quiet").await.unwrap();
    assert_eq!(report.active, "quiet");
    assert_eq!(
        report.notices,
        ["1 rule in this profile isn't on this computer, so it was left out."]
    );
    assert!(!r.desktop.state().rules.window[0].enabled);
    // The file keeps the id.
    assert!(r.desktop.get_profile("quiet").unwrap().contains(other));
}

#[tokio::test]
async fn profile_turns_a_shortcut_off_and_back() {
    let mut r = rig().await;
    add(
        &mut r,
        "format = 1\nid = \"quiet\"\nname = \"Quiet\"\n[keybinds]\n\"uq.close\" = false\n",
    )
    .await;
    let g = std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap();
    assert!(g.contains("uq:uq.close"), "{g}");
    let before = journal_len(&r);
    let report = r.desktop.apply_profile("quiet").await.unwrap();
    assert!(report.notices.is_empty(), "{report:?}");
    let entries = r.desktop.history(1000);
    let new = &entries[..entries.len() - before];
    assert_eq!(new.len(), 1, "{new:?}");
    assert_eq!(new[0].domain, "keybinds");
    assert_eq!(new[0].origin, "profile:quiet");
    assert_eq!(new[0].summary, "“Close the window” turned off");
    let g = std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap();
    assert!(!g.contains("uq:uq.close"), "{g}");
    let rows = r.desktop.explain_keybinds().await;
    assert_eq!(rows["binds.uq.close"]["source"], "profile");
    let report = r.desktop.profile_off().await.unwrap();
    assert!(report.kept.is_empty(), "{report:?}");
    let g = std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap();
    assert!(g.contains("uq:uq.close"), "{g}");
}

#[tokio::test]
async fn unknown_shortcut_id_is_a_notice() {
    let mut r = rig().await;
    add(
        &mut r,
        "format = 1\nid = \"quiet\"\nname = \"Quiet\"\n[keybinds]\n\"uq.close\" = false\n\"k-ffff\" = true\n",
    )
    .await;
    let report = r.desktop.apply_profile("quiet").await.unwrap();
    assert_eq!(report.notices.len(), 1, "{report:?}");
    assert!(report.notices[0].contains("shortcut"), "{report:?}");
}
