//! The Wallpaper page's logic: [`WallpaperState`] talks to uqd (`org.uq.Desktop1`) and is plain
//! Rust, tested against a fake uqd. `crate::wallpaper_model` is the thin QML object around it.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::client::{self, DesktopProxy};
use uq_core::settings::Wallpaper;

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

/// The result of an [`Op`]: the state to show, and uqd's message when it was refused.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// `{outputs, problems, problemsSkipped}`, or `{}` when uqd can't be read.
    pub state: Value,
    pub error: Option<String>,
}

pub struct WallpaperState {
    conn: zbus::Connection,
    /// Writes read-modify-write the section, so one at a time.
    write: Mutex<()>,
    scan: fn(&Wallpaper) -> BTreeMap<String, String>,
    scan_timeout: Duration,
}

/// The text to show for a failed call: uqd's own message when it sent one.
fn message(e: &zbus::Error) -> String {
    match e {
        zbus::Error::MethodError(_, Some(text), _) => text.clone(),
        zbus::Error::FDO(inner) => inner.to_string(),
        other => other.to_string(),
    }
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

    /// Do `op`, then read the state back. A refusal comes back as `error` with the state as uqd
    /// has it, so the controls snap back to the last good value.
    pub async fn run(&self, op: Op) -> Outcome {
        let done = {
            let _guard = self.write.lock().await;
            self.apply(op).await
        };
        let error = done
            .err()
            .filter(|e| !client::no_owner(e))
            .map(|e| message(&e));
        match self.snapshot().await {
            Ok(state) => Outcome { state, error },
            Err(e) => Outcome {
                state: json!({}),
                error: error.or_else(|| (!client::no_owner(&e)).then(|| message(&e))),
            },
        }
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
        let mut section: Value = serde_json::from_str(&proxy.get_section("wallpaper").await?)
            .map_err(|e| zbus::Error::Failure(format!("uqd sent bad JSON: {e}")))?;
        let outputs = section
            .as_object_mut()
            .ok_or_else(|| bad("the wallpaper section isn't an object"))?
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
                outputs.insert(output, merged);
            }
            None => {
                outputs.remove(&output);
            }
        }
        proxy.set_section("wallpaper", &section.to_string()).await
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let text = proxy.get_section("wallpaper").await?;
        let section: Value = serde_json::from_str(&text)
            .map_err(|e| zbus::Error::Failure(format!("uqd sent bad JSON: {e}")))?;
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
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;
    use uq_core::client::DESKTOP_NAME;
    use uq_core::settings::WallpaperEntry;
    use uq_core::test_bus::{connect, private_bus};

    #[derive(Debug, zbus::DBusError)]
    #[zbus(prefix = "org.uq.Desktop1.Error")]
    enum FakeError {
        #[zbus(error)]
        ZBus(zbus::Error),
        InvalidSetting(String),
    }

    struct Fake {
        section: Arc<StdMutex<Value>>,
    }

    #[zbus::interface(name = "org.uq.Desktop1")]
    impl Fake {
        async fn get_section(&self, _domain: &str) -> String {
            let read = self.section.lock().unwrap().to_string();
            // A slow read, so an unserialised read-modify-write would lose an update.
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            read
        }
        /// Like uqd: the interval must be 10..=86400.
        fn set_section(&self, _domain: &str, json: &str) -> Result<(), FakeError> {
            let v: Value =
                serde_json::from_str(json).map_err(|e| FakeError::InvalidSetting(e.to_string()))?;
            for (name, e) in v["outputs"].as_object().into_iter().flatten() {
                if let Some(i) = e["interval"].as_i64()
                    && !(10..=86400).contains(&i)
                {
                    return Err(FakeError::InvalidSetting(format!(
                        "{name}: interval {i} is outside 10 to 86400"
                    )));
                }
            }
            *self.section.lock().unwrap() = v;
            Ok(())
        }
    }

    struct Rig {
        _bus: uq_core::test_bus::Bus,
        _uqd: zbus::Connection,
        state: WallpaperState,
        section: Arc<StdMutex<Value>>,
    }

    async fn rig(section: Value) -> Rig {
        let bus = private_bus();
        let section = Arc::new(StdMutex::new(section));
        let uqd = connect(&bus).await;
        uqd.object_server()
            .at(
                "/org/uq/Desktop1",
                Fake {
                    section: section.clone(),
                },
            )
            .await
            .unwrap();
        uqd.request_name(DESKTOP_NAME).await.unwrap();
        let state = WallpaperState::new(connect(&bus).await);
        Rig {
            _bus: bus,
            _uqd: uqd,
            state,
            section,
        }
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
            WallpaperState::with_scan(r.state.conn.clone(), stall, Duration::from_millis(100));
        let started = std::time::Instant::now();
        let out = state
            .run(set("DP-1", json!({"fit": "fill", "interval": 5})))
            .await;
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(
            out.error.as_deref(),
            Some("DP-1: interval 5 is outside 10 to 86400"),
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
            r.state
                .run(patch("DP-1", json!({"image": "/usr/share/b.png"}))),
            r.state.run(patch("DP-1", json!({"fit": "center"})))
        );
        assert_eq!((a.error, b.error), (None, None));
        let section = r.section.lock().unwrap().clone();
        let dp1 = &section["outputs"]["DP-1"];
        assert_eq!(dp1["image"], "/usr/share/b.png");
        assert_eq!(dp1["fit"], "center");
        assert_eq!(dp1["folder"], "/usr/share/pics", "untouched fields stay");

        // A display without an entry starts from the "*" entry.
        let out = r.state.run(patch("DP-9", json!({"fit": "tile"}))).await;
        assert_eq!(
            out.state["outputs"]["DP-9"],
            json!({"image": "/usr/share/a.png", "fit": "tile", "interval": 300})
        );
    }

    #[tokio::test]
    async fn problems_follow_the_state() {
        let r = rig(json!({"outputs": {"*": {"image": "/nonexistent/x.png"}}})).await;
        let out = r.state.run(Op::Refresh).await;
        assert_eq!(
            out.state["problems"]["*"],
            "The image can't be read: /nonexistent/x.png"
        );
    }

    #[tokio::test]
    async fn set_entry_keeps_other_outputs() {
        let r = rig(two_outputs()).await;
        let entry = json!({"image": "/usr/share/b.png", "fit": "center", "interval": 300});
        let out = r.state.run(set("DP-2", entry.clone())).await;
        assert_eq!(out.error, None);
        let outputs = &out.state["outputs"];
        assert_eq!(outputs["DP-2"], entry);
        assert_eq!(outputs["*"], two_outputs()["outputs"]["*"]);
        assert_eq!(outputs["DP-1"], two_outputs()["outputs"]["DP-1"]);
        assert_eq!(r.section.lock().unwrap()["outputs"], *outputs);

        // Replacing an entry replaces all of it.
        let out = r
            .state
            .run(set("DP-1", json!({"fit": "fit", "interval": 10})))
            .await;
        assert_eq!(
            out.state["outputs"]["DP-1"],
            json!({"fit": "fit", "interval": 10})
        );
        assert_eq!(out.state["outputs"]["DP-2"], entry);
    }

    #[tokio::test]
    async fn clear_entry_removes_only_that_output() {
        let r = rig(two_outputs()).await;
        let out = r
            .state
            .run(Op::ClearEntry {
                output: "DP-1".into(),
            })
            .await;
        assert_eq!(out.error, None);
        assert_eq!(
            out.state["outputs"],
            json!({"*": two_outputs()["outputs"]["*"]})
        );
        // Clearing an absent output is harmless.
        let out = r
            .state
            .run(Op::ClearEntry {
                output: "HDMI-9".into(),
            })
            .await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["outputs"].as_object().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn concurrent_entry_changes_both_land() {
        let r = rig(json!({"outputs": {}})).await;
        let (a, b) = tokio::join!(
            r.state
                .run(set("DP-1", json!({"fit": "fill", "interval": 300}))),
            r.state
                .run(set("DP-2", json!({"fit": "tile", "interval": 300})))
        );
        assert_eq!((a.error, b.error), (None, None));
        let section = r.section.lock().unwrap().clone();
        assert_eq!(section["outputs"].as_object().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn error_restores_state() {
        let r = rig(two_outputs()).await;
        let before = r.state.run(Op::Refresh).await.state;
        let out = r
            .state
            .run(set("DP-1", json!({"fit": "fill", "interval": 5})))
            .await;
        assert_eq!(
            out.error.as_deref(),
            Some("DP-1: interval 5 is outside 10 to 86400")
        );
        assert_eq!(out.state, before);
    }

    #[tokio::test]
    async fn uqd_down_gives_empty_state_without_an_error() {
        let bus = private_bus();
        let state = WallpaperState::new(connect(&bus).await);
        let out = state.run(Op::Refresh).await;
        assert_eq!(
            out,
            Outcome {
                state: json!({}),
                error: None
            }
        );
    }
}
