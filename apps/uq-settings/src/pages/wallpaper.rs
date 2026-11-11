//! The Wallpaper page's logic: [`WallpaperState`] talks to uqd (`org.uq.Desktop1`) and is plain
//! Rust, tested against the fake uqd. `crate::wallpaper_model` is the thin QML object around it.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::client::DesktopProxy;
use uq_core::settings::Wallpaper;

use crate::pages::runner::{PageLogic, parse_json, patch_section};

/// The file types the shell's slideshow picks up.
const IMAGE_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "webp", "jxl", "avif"];
/// The most folder entries looked at per folder.
const MAX_SCANNED: usize = 5000;
/// How long the disk checks may take before the page is shown without them.
const SCAN_TIMEOUT: Duration = Duration::from_secs(2);

/// What the page asks for.
#[derive(Debug, Clone)]
pub enum Op {
    /// Just read everything.
    Refresh,
    /// Replace one output's entry (`"*"` = all displays) with `entry`.
    SetEntry { output: String, entry: Value },
    /// Change some fields of one output's entry: `patch` (an object) is merged over the entry as
    /// uqd has it now (or, for a display without one, over the `"*"` entry), so two quick edits
    /// to different fields both land.
    PatchEntry { output: String, patch: Value },
    /// Remove one output's entry.
    ClearEntry { output: String },
}

/// What is wrong with putting `fit` on `output`, in plain words, or `None`. Only the All
/// displays entry can span: the other screens each draw their own picture.
pub fn span_refusal(output: &str, fit: Option<&Value>) -> Option<&'static str> {
    (output != "*" && fit.and_then(Value::as_str) == Some("span"))
        .then_some("Span works only for All displays")
}

pub struct WallpaperState {
    conn: zbus::Connection,
    /// Writes read-modify-write the section, so one at a time.
    write: Mutex<()>,
    scan: fn(&Wallpaper) -> BTreeMap<String, String>,
    scan_timeout: Duration,
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// A regular file (or a link to one) that can be opened. Anything else, such as a FIFO, is
/// refused without being opened, since opening it could block forever.
fn readable_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file()) && std::fs::File::open(path).is_ok()
}

/// What is wrong with each output's wallpaper, by output name. This mirrors the shell: a folder
/// wins over an image (`shell/wallpaper/Wallpaper.qml`), so only the one in use is checked. Outputs that are fine are left out.
pub fn problems(w: &Wallpaper) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (name, entry) in &w.outputs {
        if let Some(folder) = &entry.folder {
            let has_image = std::fs::read_dir(folder).is_ok_and(|dir| {
                dir.flatten()
                    .take(MAX_SCANNED)
                    .any(|f| is_image(&f.path()) && readable_file(&f.path()))
            });
            if !has_image {
                out.insert(
                    name.clone(),
                    format!("The folder has no readable images: {}", folder.display()),
                );
            }
        } else if let Some(image) = &entry.image
            && !readable_file(image)
        {
            out.insert(
                name.clone(),
                format!("The image can't be read: {}", image.display()),
            );
        }
    }
    out
}

/// The local path a file or folder dialog's `file://` URL names, percent-decoded. A bare
/// absolute path passes through; anything else (another scheme, a remote host) gives `None`.
pub fn local_path(url: &str) -> Option<String> {
    let rest = match url.strip_prefix("file://") {
        Some(rest) => rest,
        None if url.starts_with('/') => url,
        None => return None,
    };
    // `file:///x` and `file://localhost/x` are local; `file://host/x` is not.
    let path = rest.strip_prefix("localhost").unwrap_or(rest);
    if !path.starts_with('/') {
        return None;
    }
    let decoded = percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .ok()?;
    Some(decoded.into_owned())
}

impl WallpaperState {
    pub fn new(conn: zbus::Connection) -> Self {
        Self::with_scan(conn, problems, SCAN_TIMEOUT)
    }

    /// Like [`Self::new`], with the disk check and its time limit chosen (tests).
    pub fn with_scan(
        conn: zbus::Connection,
        scan: fn(&Wallpaper) -> BTreeMap<String, String>,
        scan_timeout: Duration,
    ) -> Self {
        Self {
            conn,
            write: Mutex::new(()),
            scan,
            scan_timeout,
        }
    }
}

/// The state is `{outputs, problems, problemsSkipped}`.
impl PageLogic for WallpaperState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let (output, change) = match op {
            Op::Refresh => return Ok(()),
            Op::SetEntry { output, entry } => (output, Some((entry, false))),
            Op::PatchEntry { output, patch } => (output, Some((patch, true))),
            Op::ClearEntry { output } => (output, None),
        };
        let bad = |what: &str| zbus::Error::Failure(what.to_string());
        // Refuse before uqd does, in plain words.
        if let Some((entry, _)) = &change
            && let Some(why) = span_refusal(&output, entry.get("fit"))
        {
            return Err(bad(why));
        }
        patch_section(&proxy, "wallpaper", |section| {
            let outputs = section
                .entry("outputs")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or_else(|| bad("the wallpaper outputs aren't an object"))?;
            match change {
                Some((entry, false)) => {
                    outputs.insert(output, entry);
                }
                Some((patch, true)) => {
                    let mut merged = outputs
                        .get(&output)
                        .or_else(|| outputs.get("*"))
                        .cloned()
                        .unwrap_or_else(|| json!({}));
                    let (Some(fields), Some(changes)) = (merged.as_object_mut(), patch.as_object())
                    else {
                        return Err(bad("a wallpaper entry must be an object"));
                    };
                    fields.extend(changes.iter().map(|(k, v)| (k.clone(), v.clone())));
                    // A display that had no entry copies "*", and a spanning "*" can't be
                    // copied: its own picture is filled instead, unless the patch chose a fit.
                    if output != "*"
                        && !changes.contains_key("fit")
                        && fields.get("fit").and_then(Value::as_str) == Some("span")
                    {
                        fields.insert("fit".into(), json!("fill"));
                    }
                    outputs.insert(output, merged);
                }
                None => {
                    outputs.remove(&output);
                }
            }
            Ok(())
        })
        .await
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let section = parse_json(&proxy.get_section("wallpaper").await?)?;
        let parsed: Wallpaper = serde_json::from_value(section.clone()).unwrap_or_default();
        // Disk checks can block on a stalled mount: off the async threads, and not waited for
        // beyond the time limit (the page then shows without problems, `problemsSkipped` set).
        let scan = self.scan;
        let checked = tokio::time::timeout(
            self.scan_timeout,
            tokio::task::spawn_blocking(move || scan(&parsed)),
        )
        .await;
        let (problems, skipped) = match checked {
            Ok(Ok(found)) => (found, false),
            Ok(Err(_)) | Err(_) => (BTreeMap::new(), true),
        };
        Ok(json!({
            "outputs": section.get("outputs").cloned().unwrap_or_else(|| json!({})),
            "problems": problems,
            "problemsSkipped": skipped,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::fake_uqd;
    use crate::pages::runner::{Outcome, run};
    use uq_core::settings::WallpaperEntry;
    use uq_core::state::State;
    use uq_core::test_bus::{connect, private_bus};

    struct Rig {
        r: fake_uqd::Rig,
        state: WallpaperState,
    }

    impl Rig {
        fn section(&self) -> Value {
            self.r
                .state
                .lock()
                .unwrap()
                .settings
                .section("wallpaper")
                .unwrap()
        }
    }

    async fn rig(section: Value) -> Rig {
        let mut state = State::default();
        state.settings.set_section("wallpaper", section).unwrap();
        let r = fake_uqd::rig(state).await;
        let state = WallpaperState::new(r.client.clone());
        Rig { r, state }
    }

    fn two_outputs() -> Value {
        json!({"outputs": {
            "*": {"image": "/usr/share/a.png", "fit": "fill", "interval": 300},
            "DP-1": {"folder": "/usr/share/pics", "fit": "tile", "interval": 60, "color": "#102030"},
        }})
    }

    fn set(output: &str, entry: Value) -> Op {
        Op::SetEntry {
            output: output.into(),
            entry,
        }
    }

    #[test]
    fn span_offered_only_for_all_displays() {
        assert_eq!(span_refusal("*", Some(&json!("span"))), None);
        assert_eq!(
            span_refusal("DP-1", Some(&json!("span"))),
            Some("Span works only for All displays")
        );
        assert_eq!(span_refusal("DP-1", Some(&json!("fill"))), None);
        assert_eq!(span_refusal("DP-1", None), None);
    }

    #[tokio::test]
    async fn span_is_refused_for_one_display_and_accepted_for_all() {
        let r = rig(two_outputs()).await;
        let patch = |output: &str, patch: Value| Op::PatchEntry {
            output: output.into(),
            patch,
        };
        let out = run(&r.state, patch("DP-1", json!({"fit": "span"}))).await;
        assert_eq!(
            out.error.as_deref(),
            Some("Span works only for All displays")
        );
        assert_eq!(r.section()["outputs"]["DP-1"]["fit"], "tile");
        let entry = json!({"fit": "span", "interval": 300});
        let out = run(&r.state, set("DP-1", entry)).await;
        assert_eq!(
            out.error.as_deref(),
            Some("Span works only for All displays")
        );

        let out = run(&r.state, patch("*", json!({"fit": "span"}))).await;
        assert_eq!(out.error, None);
        assert_eq!(r.section()["outputs"]["*"]["fit"], "span");
        // A display that starts from a spanning "*" gets its own picture filled, not spanned.
        let out = run(
            &r.state,
            patch("DP-9", json!({"image": "/usr/share/c.png"})),
        )
        .await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["outputs"]["DP-9"]["fit"], "fill");
        assert_eq!(out.state["outputs"]["DP-9"]["image"], "/usr/share/c.png");
    }

    #[test]
    fn dialog_urls_become_local_paths() {
        assert_eq!(
            local_path("file:///home/a/My%20Pics/%C3%A9t%C3%A9.png").as_deref(),
            Some("/home/a/My Pics/\u{e9}t\u{e9}.png")
        );
        assert_eq!(local_path("file:///a%23b%25").as_deref(), Some("/a#b%"));
        assert_eq!(local_path("file://localhost/x").as_deref(), Some("/x"));
        assert_eq!(
            local_path("/already/a/path").as_deref(),
            Some("/already/a/path")
        );
        assert_eq!(local_path("file://host/x"), None);
        assert_eq!(local_path("https://example.com/x.png"), None);
        assert_eq!(local_path("file:///bad%FF"), None);
        assert_eq!(local_path(""), None);
    }

    #[test]
    fn missing_image_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.png");
        std::fs::write(&good, b"x").unwrap();
        let gone = dir.path().join("gone.png");
        let empty = dir.path().join("empty");
        std::fs::create_dir(&empty).unwrap();
        std::fs::write(empty.join("notes.txt"), b"x").unwrap();
        let pics = dir.path().join("pics");
        std::fs::create_dir(&pics).unwrap();
        std::fs::write(pics.join("a.JPG"), b"x").unwrap();

        let entry = |image: Option<&Path>, folder: Option<&Path>| WallpaperEntry {
            image: image.map(Into::into),
            folder: folder.map(Into::into),
            ..WallpaperEntry::default()
        };
        let w = Wallpaper {
            outputs: BTreeMap::from([
                ("*".into(), entry(Some(&good), None)),
                ("DP-1".into(), entry(Some(&gone), None)),
                ("DP-2".into(), entry(None, Some(&empty))),
                ("DP-3".into(), entry(None, Some(&pics))),
                ("DP-4".into(), entry(None, Some(&dir.path().join("nope")))),
                ("eDP-1".into(), entry(None, None)),
            ]),
        };
        let p = problems(&w);
        assert_eq!(
            p.keys().map(String::as_str).collect::<Vec<_>>(),
            ["DP-1", "DP-2", "DP-4"]
        );
        assert_eq!(
            p["DP-1"],
            format!("The image can't be read: {}", gone.display())
        );
        assert_eq!(
            p["DP-2"],
            format!("The folder has no readable images: {}", empty.display())
        );
    }

    #[test]
    fn a_fifo_named_like_an_image_does_not_hang() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("x.png");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        let entry = |image: Option<&Path>, folder: Option<&Path>| WallpaperEntry {
            image: image.map(Into::into),
            folder: folder.map(Into::into),
            ..WallpaperEntry::default()
        };
        let w = Wallpaper {
            outputs: BTreeMap::from([
                ("DP-1".into(), entry(None, Some(dir.path()))),
                ("DP-2".into(), entry(Some(&fifo), None)),
            ]),
        };
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || tx.send(problems(&w)).unwrap());
        let p = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the scan hung on a FIFO");
        assert_eq!(
            p.keys().map(String::as_str).collect::<Vec<_>>(),
            ["DP-1", "DP-2"]
        );
    }

    #[tokio::test]
    async fn a_stalled_scan_still_delivers_the_outcome() {
        fn stall(_: &Wallpaper) -> BTreeMap<String, String> {
            std::thread::sleep(Duration::from_secs(3));
            BTreeMap::new()
        }
        let r = rig(two_outputs()).await;
        let state =
            WallpaperState::with_scan(r.r.client.clone(), stall, Duration::from_millis(100));
        let started = std::time::Instant::now();
        let out = run(&state, set("DP-1", json!({"fit": "fill", "interval": 5}))).await;
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(
            out.error.as_deref(),
            Some("invalid value for wallpaper.outputs.DP-1.interval: must be between 10 and 86400"),
            "a refused write's message still arrives"
        );
        assert_eq!(out.state["problemsSkipped"], true);
        assert_eq!(out.state["problems"], json!({}));
        assert_eq!(out.state["outputs"]["DP-1"]["fit"], "tile");
    }

    #[tokio::test]
    async fn patches_merge_into_the_current_entry() {
        let r = rig(two_outputs()).await;
        let patch = |output: &str, patch: Value| Op::PatchEntry {
            output: output.into(),
            patch,
        };
        let (a, b) = tokio::join!(
            run(
                &r.state,
                patch("DP-1", json!({"image": "/usr/share/b.png"}))
            ),
            run(&r.state, patch("DP-1", json!({"fit": "center"})))
        );
        assert_eq!((a.error, b.error), (None, None));
        let section = r.section();
        let dp1 = &section["outputs"]["DP-1"];
        assert_eq!(dp1["image"], "/usr/share/b.png");
        assert_eq!(dp1["fit"], "center");
        assert_eq!(dp1["folder"], "/usr/share/pics", "untouched fields stay");

        // A display without an entry starts from the "*" entry.
        let out = run(&r.state, patch("DP-9", json!({"fit": "tile"}))).await;
        assert_eq!(
            out.state["outputs"]["DP-9"],
            json!({"image": "/usr/share/a.png", "fit": "tile", "interval": 300})
        );
    }

    #[tokio::test]
    async fn problems_follow_the_state() {
        let r = rig(json!({"outputs": {"*": {"image": "/nonexistent/x.png"}}})).await;
        let out = run(&r.state, Op::Refresh).await;
        assert_eq!(
            out.state["problems"]["*"],
            "The image can't be read: /nonexistent/x.png"
        );
    }

    #[tokio::test]
    async fn set_entry_keeps_other_outputs() {
        let r = rig(two_outputs()).await;
        let entry = json!({"image": "/usr/share/b.png", "fit": "center", "interval": 300});
        let out = run(&r.state, set("DP-2", entry.clone())).await;
        assert_eq!(out.error, None);
        let outputs = &out.state["outputs"];
        assert_eq!(outputs["DP-2"], entry);
        assert_eq!(outputs["*"], two_outputs()["outputs"]["*"]);
        assert_eq!(outputs["DP-1"], two_outputs()["outputs"]["DP-1"]);
        assert_eq!(r.section()["outputs"], *outputs);

        // Replacing an entry replaces all of it.
        let out = run(&r.state, set("DP-1", json!({"fit": "fit", "interval": 10}))).await;
        assert_eq!(
            out.state["outputs"]["DP-1"],
            json!({"fit": "fit", "interval": 10})
        );
        assert_eq!(out.state["outputs"]["DP-2"], entry);
    }

    #[tokio::test]
    async fn clear_entry_removes_only_that_output() {
        let r = rig(two_outputs()).await;
        let out = run(
            &r.state,
            Op::ClearEntry {
                output: "DP-1".into(),
            },
        )
        .await;
        assert_eq!(out.error, None);
        assert_eq!(
            out.state["outputs"],
            json!({"*": two_outputs()["outputs"]["*"]})
        );
        // Clearing an absent output is harmless.
        let out = run(
            &r.state,
            Op::ClearEntry {
                output: "HDMI-9".into(),
            },
        )
        .await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["outputs"].as_object().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn concurrent_entry_changes_both_land() {
        let r = rig(json!({"outputs": {}})).await;
        let (a, b) = tokio::join!(
            run(
                &r.state,
                set("DP-1", json!({"fit": "fill", "interval": 300}))
            ),
            run(
                &r.state,
                set("DP-2", json!({"fit": "tile", "interval": 300}))
            )
        );
        assert_eq!((a.error, b.error), (None, None));
        let section = r.section();
        assert_eq!(section["outputs"].as_object().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn error_restores_state() {
        let r = rig(two_outputs()).await;
        let before = run(&r.state, Op::Refresh).await.state;
        let out = run(&r.state, set("DP-1", json!({"fit": "fill", "interval": 5}))).await;
        assert_eq!(
            out.error.as_deref(),
            Some("invalid value for wallpaper.outputs.DP-1.interval: must be between 10 and 86400")
        );
        assert_eq!(out.state, before);
    }

    #[tokio::test]
    async fn uqd_down_gives_empty_state_without_an_error() {
        let bus = private_bus();
        let state = WallpaperState::new(connect(&bus).await);
        let out = run(&state, Op::Refresh).await;
        assert_eq!(
            out,
            Outcome {
                state: json!({}),
                error: None
            }
        );
    }

    /// Every field of an output's entry keeps its value: a fresh page object on the same uqd
    /// reads back what each `Op` wrote, so no control resets itself.
    #[tokio::test]
    async fn every_field_round_trips() {
        let rig = rig(json!({})).await;
        let image = json!({"image": "/usr/share/b.png", "fit": "tile", "interval": 600,
                           "color": "#102030"});
        let folder = json!({"folder": "/usr/share/pics", "fit": "center", "interval": 45});
        for op in [
            set("*", image.clone()),
            set("DP-1", folder.clone()),
            // A patch changes one field and keeps the rest.
            Op::PatchEntry {
                output: "*".into(),
                patch: json!({"fit": "fit"}),
            },
            set("HDMI-A-1", json!({"image": "/usr/share/c.png"})),
            Op::ClearEntry {
                output: "HDMI-A-1".into(),
            },
        ] {
            let out = run(&rig.state, op).await;
            assert_eq!(out.error, None);
        }
        let fresh = WallpaperState::new(rig.r.client.clone());
        let out = run(&fresh, Op::Refresh).await;
        let outputs = &out.state["outputs"];
        assert_eq!(
            outputs["*"],
            json!({"image": "/usr/share/b.png", "fit": "fit",
                                        "interval": 600, "color": "#102030"})
        );
        assert_eq!(
            outputs["DP-1"],
            json!({"folder": "/usr/share/pics", "fit": "center",
                                           "interval": 45})
        );
        assert!(outputs.get("HDMI-A-1").is_none(), "{outputs}");
    }
}
