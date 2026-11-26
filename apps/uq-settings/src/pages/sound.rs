//! The Sound page (spec §2.6): outputs and inputs, which one is the default, each one's volume
//! and mute, a sound card's profile, each app's volume and which output it plays through, and a
//! test sound.
//!
//! **Backend.** The spec names the `pipewire` crate in a worker thread. That crate needs
//! `libpipewire` and `libspa` headers at build time (through bindgen) and isn't in the lock
//! file; linking it would also tie this app to the PipeWire version it was built against. So the
//! page uses PipeWire's own tools instead, which every PipeWire system has and which keep their
//! command-line interface stable:
//!
//! - reading: `pw-dump` (the whole graph as JSON, the same data the crate would give), and
//!   `pw-dump --monitor` while the page is open, to follow changes;
//! - volume, mute, default output or input, and card profile: `wpctl` (WirePlumber, the session
//!   manager that owns those settings and remembers them);
//! - moving an app to another output: `pw-metadata` (`target.object`, the key WirePlumber
//!   follows, the same thing `pactl move-sink-input` sets);
//! - the test sound: `pw-play`.
//!
//! Every tool runs with an argument list (never a shell) and a time limit ([`tools`]). Volumes
//! are shown the way `wpctl` and the shell's volume slider show them (the cube root of
//! PipeWire's linear volume), so the numbers agree everywhere. Device and app names come from
//! the devices and apps and are shown through UqLabel (plain text) only.

pub(crate) mod tools;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::sync::{Mutex, Notify};

use super::runner::PageLogic;
use super::sysbus::{Service, failure};
use tools::{ToolError, Tools};
mod graph;
pub use graph::*;

pub const PIPEWIRE: Service = Service {
    name: "Sound settings",
    package: "pipewire",
};
pub const WIREPLUMBER: Service = Service {
    name: "Sound settings",
    package: "wireplumber",
};

/// Test sounds, best first (under the page's root): the freedesktop sound theme's.
const TEST_SOUNDS: [&str; 2] = [
    "usr/share/sounds/freedesktop/stereo/audio-test-signal.oga",
    "usr/share/sounds/freedesktop/stereo/bell.oga",
];

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    /// Make output or input `id` the default.
    SetDefault(u32),
    /// 0 to 100.
    SetVolume(u32, u32),
    SetMute(u32, bool),
    /// Card `id`'s profile `index`.
    SetProfile(u32, u32),
    /// Play app stream `id` through output `serial`; 0 follows the default output again.
    Route(u32, u64),
    /// Play the test sound on output `id`.
    Test(u32),
}

/// One node or device as read from `pw-dump`.
#[derive(Debug, Clone, PartialEq)]
struct Object {
    id: u32,
    kind: String,
    class: String,
    props: Value,
    params: Value,
}

fn props_of(o: &Value) -> Value {
    o.pointer("/info/props")
        .or_else(|| o.get("props"))
        .cloned()
        .unwrap_or(Value::Null)
}

/// The object types [`parse`] reads: a change to anything else doesn't change the page.
const SHOWN_TYPES: [&str; 4] = [
    "PipeWire:Interface:Node",
    "PipeWire:Interface:Device",
    "PipeWire:Interface:Link",
    "PipeWire:Interface:Metadata",
];

/// The largest single `pw-dump --monitor` update kept while it is read (a whole graph is a few
/// hundred KiB); past it the update counts as a change and is dropped.
const MAX_UPDATE: usize = 32 << 20;

/// Splits `pw-dump --monitor`'s output into its updates (one pretty-printed JSON array each,
/// closed by a `]` line) and says which concern the page.
#[derive(Default)]
pub struct MonitorUpdates {
    text: String,
    /// The ids of the objects of [`SHOWN_TYPES`] seen so far, so their removal counts.
    shown: std::collections::HashSet<u64>,
}

impl MonitorUpdates {
    /// Take one line of output; true when it ends an update that adds, changes or removes an
    /// object the page shows (or one that can't be read, to be safe).
    pub fn line(&mut self, line: &str) -> bool {
        self.text.push_str(line);
        self.text.push('\n');
        if line != "]" {
            if self.text.len() > MAX_UPDATE {
                self.text.clear();
                return true;
            }
            return false;
        }
        let text = std::mem::take(&mut self.text);
        let Ok(objects) = serde_json::from_str::<Vec<Value>>(&text) else {
            return true;
        };
        let mut concerns = false;
        for o in &objects {
            let Some(id) = o["id"].as_u64() else {
                continue;
            };
            let kind = o.get("type").and_then(Value::as_str);
            if kind.is_some_and(|k| SHOWN_TYPES.contains(&k)) {
                self.shown.insert(id);
                concerns = true;
            } else if kind.is_none() && o.get("info").is_some_and(Value::is_null) {
                // `{"id": N, "info": null}`: object N is gone.
                concerns |= self.shown.remove(&id);
            } else {
                // A client, port, module…; an id once used by a shown object can be reused.
                self.shown.remove(&id);
            }
        }
        concerns
    }
}

fn text(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| v[*k].as_str().filter(|s| !s.is_empty()))
        .unwrap_or("")
        .to_string()
}

pub struct SoundState {
    tools: Arc<dyn Tools>,
    root: PathBuf,
    write: Mutex<()>,
    notice: StdMutex<(u64, String)>,
}

fn argv(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

/// A tool's failure as the page shows it.
fn tool_error(e: ToolError) -> zbus::Error {
    match e {
        ToolError::Missing(p) if p.starts_with("pw-") => failure(PIPEWIRE.missing_text()),
        ToolError::Missing(p) if p == "wpctl" => failure(WIREPLUMBER.missing_text()),
        ToolError::Missing(p) => failure(format!("{p} isn't installed.")),
        ToolError::TimedOut => failure("The sound service didn't answer in time."),
        ToolError::Failed(text) => failure(text),
    }
}

impl SoundState {
    pub fn new(_session: zbus::Connection) -> Self {
        Self::with(Arc::new(tools::System), PathBuf::from("/"))
    }

    pub fn with(tools: Arc<dyn Tools>, root: PathBuf) -> Self {
        Self {
            tools,
            root,
            write: Mutex::new(()),
            notice: StdMutex::new((0, String::new())),
        }
    }

    fn notice(&self, text: &str) {
        let mut n = self.notice.lock().unwrap();
        *n = (n.0 + 1, text.to_string());
    }

    async fn graph(&self) -> zbus::Result<Graph> {
        let out = self
            .tools
            .run(argv(&["pw-dump"]))
            .await
            .map_err(tool_error)?;
        parse(&out).map_err(failure)
    }

    async fn wpctl(&self, args: &[&str]) -> zbus::Result<()> {
        let mut a = vec!["wpctl"];
        a.extend_from_slice(args);
        self.tools
            .run(argv(&a))
            .await
            .map(|_| ())
            .map_err(tool_error)
    }

    fn test_sound(&self) -> Option<PathBuf> {
        TEST_SOUNDS
            .iter()
            .map(|p| self.root.join(p))
            .find(|p| p.is_file())
    }
}

/// `object` must still be one of `classes` (ids are reused when devices come and go).
fn still<'a>(g: &'a Graph, id: u32, classes: &[&str]) -> zbus::Result<&'a Object> {
    g.is(id, classes)
        .ok_or_else(|| failure("That device or app is gone."))
}

const DEVICES: [&str; 2] = ["Audio/Sink", "Audio/Source"];
const STREAMS: [&str; 2] = ["Stream/Output/Audio", "Stream/Input/Audio"];

impl PageLogic for SoundState {
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
        if matches!(op, Op::Refresh) {
            return Ok(());
        }
        let g = self.graph().await?;
        match op {
            Op::Refresh => Ok(()),
            Op::SetDefault(id) => {
                still(&g, id, &DEVICES)?;
                self.wpctl(&["set-default", &id.to_string()]).await
            }
            Op::SetVolume(id, pct) => {
                still(&g, id, &[DEVICES, STREAMS].concat())?;
                let v = format!("{:.2}", f64::from(pct.min(100)) / 100.0);
                self.wpctl(&["set-volume", &id.to_string(), &v]).await
            }
            Op::SetMute(id, on) => {
                still(&g, id, &[DEVICES, STREAMS].concat())?;
                self.wpctl(&["set-mute", &id.to_string(), if on { "1" } else { "0" }])
                    .await
            }
            Op::SetProfile(id, index) => {
                let card = g
                    .get(id)
                    .filter(|o| o.class == "Audio/Device")
                    .ok_or_else(|| failure("That sound card is gone."))?;
                let known = card.params["EnumProfile"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|p| p["index"].as_u64() == Some(u64::from(index)));
                if !known {
                    return Err(failure("That card doesn't offer this profile any more."));
                }
                self.wpctl(&["set-profile", &id.to_string(), &index.to_string()])
                    .await
            }
            Op::Route(id, serial) => {
                let stream = still(&g, id, &STREAMS)?;
                let id = id.to_string();
                if serial == 0 {
                    return self
                        .tools
                        .run(argv(&[
                            "pw-metadata",
                            "-n",
                            "default",
                            "-d",
                            &id,
                            "target.object",
                        ]))
                        .await
                        .map(|_| ())
                        .map_err(tool_error);
                }
                let want = if stream.class == "Stream/Output/Audio" {
                    "Audio/Sink"
                } else {
                    "Audio/Source"
                };
                if !g
                    .objects
                    .iter()
                    .any(|o| o.class == want && Graph::serial(o) == serial)
                {
                    return Err(failure("That device is gone."));
                }
                self.tools
                    .run(argv(&[
                        "pw-metadata",
                        "-n",
                        "default",
                        &id,
                        "target.object",
                        &serial.to_string(),
                        "Spa:Id",
                    ]))
                    .await
                    .map(|_| ())
                    .map_err(tool_error)
            }
            Op::Test(id) => {
                let sink = still(&g, id, &["Audio/Sink"])?;
                let file = self.test_sound().ok_or_else(|| {
                    failure(
                        "No test sound is installed. Install the sound-theme-freedesktop package.",
                    )
                })?;
                let target = text(&sink.props, &["node.name"]);
                self.tools
                    .start(vec![
                        "pw-play".into(),
                        "--target".into(),
                        target,
                        path_arg(&file),
                    ])
                    .map_err(tool_error)?;
                self.notice("Playing a test sound…");
                Ok(())
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let mut state = match self.graph().await {
            Ok(g) => g.state(),
            Err(zbus::Error::Failure(text)) if text == PIPEWIRE.missing_text() => {
                PIPEWIRE.missing_json()
            }
            Err(e) => json!({
                "available": false,
                "name": PIPEWIRE.name,
                "package": PIPEWIRE.package,
                "text": crate::pages::runner::message(&e),
            }),
        };
        let (seq, text) = self.notice.lock().unwrap().clone();
        state["notice"] = json!({"seq": seq, "text": text});
        state["test"] = json!(self.test_sound().is_some());
        Ok(state)
    }

    async fn watch(&self, changed: Arc<Notify>) {
        // pw-dump prints the graph once, then each change; a change to what the page shows
        // means "read again". Clients and ports don't count: each refresh's own `pw-dump` is a
        // client coming and going, which made the open page read itself again every ~300 ms.
        let mut lines = self.tools.lines(argv(&["pw-dump", "--monitor"]));
        let mut updates = MonitorUpdates::default();
        while let Some(line) = lines.next().await {
            if updates.line(&line) {
                changed.notify_one();
            }
        }
        std::future::pending::<()>().await;
    }
}

fn path_arg(p: &Path) -> String {
    p.display().to_string()
}

#[cfg(test)]
mod tests;
