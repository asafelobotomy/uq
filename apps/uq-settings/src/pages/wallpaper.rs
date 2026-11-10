//! The Wallpaper page's logic: [`WallpaperState`] talks to uqd (`org.uq.Desktop1`) and is plain
//! Rust, tested against a fake uqd. `crate::wallpaper_model` is the thin QML object around it.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::client::{self, DesktopProxy};
use uq_core::settings::Wallpaper;

/// The file types the shell's slideshow picks up.
const IMAGE_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "webp", "jxl", "avif"];

/// What the page asks for.
#[derive(Debug, Clone)]
pub enum Op {
    /// Just read everything.
    Refresh,
    /// Replace one output's entry (`"*"` = all displays) with `entry`.
    SetEntry { output: String, entry: Value },
    /// Remove one output's entry.
    ClearEntry { output: String },
}

/// The result of an [`Op`]: the state to show, and uqd's message when it was refused.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// `{outputs, problems}`, or `{}` when uqd can't be read.
    pub state: Value,
    pub error: Option<String>,
}

pub struct WallpaperState {
    conn: zbus::Connection,
    /// Writes read-modify-write the section, so one at a time.
    write: Mutex<()>,
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

/// What is wrong with each output's wallpaper, by output name. A folder wins over an image (as in
/// the shell), so only the one in use is checked. Outputs that are fine are left out.
pub fn problems(w: &Wallpaper) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (name, entry) in &w.outputs {
        if let Some(folder) = &entry.folder {
            let has_image = std::fs::read_dir(folder).is_ok_and(|dir| {
                dir.flatten()
                    .any(|f| is_image(&f.path()) && std::fs::File::open(f.path()).is_ok())
            });
            if !has_image {
                out.insert(
                    name.clone(),
                    format!("The folder has no readable images: {}", folder.display()),
                );
            }
        } else if let Some(image) = &entry.image
            && !(image.is_file() && std::fs::File::open(image).is_ok())
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
        Self {
            conn,
            write: Mutex::new(()),
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
        let (output, entry) = match op {
            Op::Refresh => return Ok(()),
            Op::SetEntry { output, entry } => (output, Some(entry)),
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
        match entry {
            Some(entry) => outputs.insert(output, entry),
            None => outputs.remove(&output),
        };
        proxy.set_section("wallpaper", &section.to_string()).await
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let text = proxy.get_section("wallpaper").await?;
        let section: Value = serde_json::from_str(&text)
            .map_err(|e| zbus::Error::Failure(format!("uqd sent bad JSON: {e}")))?;
        let parsed: Wallpaper = serde_json::from_value(section.clone()).unwrap_or_default();
        // Disk checks can block on a slow mount; keep them off the async threads.
        let problems = tokio::task::spawn_blocking(move || problems(&parsed))
            .await
            .unwrap_or_default();
        Ok(json!({
            "outputs": section.get("outputs").cloned().unwrap_or_else(|| json!({})),
            "problems": problems,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_bus::{connect, private_bus};
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;
    use uq_core::client::DESKTOP_NAME;
    use uq_core::settings::WallpaperEntry;

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
        _bus: crate::test_bus::Bus,
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
