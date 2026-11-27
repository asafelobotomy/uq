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
mod tests;
