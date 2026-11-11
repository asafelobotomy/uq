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

/// The parts of a dump the page uses.
#[derive(Debug, Default)]
pub struct Graph {
    objects: Vec<Object>,
    /// `(output node, input node)` of each link.
    links: Vec<(u32, u32)>,
    /// The `default` metadata: `(subject, key) → value`.
    metadata: HashMap<(u32, String), Value>,
}

fn props_of(o: &Value) -> Value {
    o.pointer("/info/props")
        .or_else(|| o.get("props"))
        .cloned()
        .unwrap_or(Value::Null)
}

/// Parse `pw-dump`'s output.
pub fn parse(text: &str) -> Result<Graph, String> {
    let all: Vec<Value> = serde_json::from_str(text)
        .map_err(|e| format!("Sound settings got unreadable data: {e}"))?;
    let mut g = Graph::default();
    for o in &all {
        let Some(id) = o["id"].as_u64().and_then(|i| u32::try_from(i).ok()) else {
            continue;
        };
        let kind = o["type"].as_str().unwrap_or("");
        match kind {
            "PipeWire:Interface:Node" | "PipeWire:Interface:Device" => {
                let props = props_of(o);
                g.objects.push(Object {
                    id,
                    kind: kind.rsplit(':').next().unwrap_or("").to_string(),
                    class: props["media.class"].as_str().unwrap_or("").to_string(),
                    props,
                    params: o.pointer("/info/params").cloned().unwrap_or(Value::Null),
                });
            }
            "PipeWire:Interface:Link" => {
                let out = o.pointer("/info/output-node-id").and_then(Value::as_u64);
                let inp = o.pointer("/info/input-node-id").and_then(Value::as_u64);
                if let (Some(a), Some(b)) = (out, inp) {
                    g.links.push((a as u32, b as u32));
                }
            }
            "PipeWire:Interface:Metadata" => {
                if props_of(o)["metadata.name"] != "default" {
                    continue;
                }
                for m in o["metadata"].as_array().into_iter().flatten() {
                    let (Some(subject), Some(key)) = (m["subject"].as_u64(), m["key"].as_str())
                    else {
                        continue;
                    };
                    g.metadata
                        .insert((subject as u32, key.to_string()), m["value"].clone());
                }
            }
            _ => {}
        }
    }
    Ok(g)
}

/// A metadata value that names a node: `{"name": …}` (parsed or as text) or the text itself.
fn named(v: &Value) -> Option<String> {
    match v {
        Value::Object(o) => o.get("name")?.as_str().map(str::to_string),
        Value::String(s) => serde_json::from_str::<Value>(s)
            .ok()
            .and_then(|j| j.get("name")?.as_str().map(str::to_string))
            .or_else(|| Some(s.clone())),
        _ => None,
    }
}

/// The volume as shown (0–100, the cube root of the linear average) and the mute.
fn volume(o: &Object) -> (u32, bool) {
    let props = o.params["Props"]
        .as_array()
        .and_then(|a| a.iter().find(|p| p.get("channelVolumes").is_some()));
    let Some(p) = props else {
        return (100, false);
    };
    let vols: Vec<f64> = p["channelVolumes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_f64)
        .collect();
    let avg = if vols.is_empty() {
        1.0
    } else {
        vols.iter().sum::<f64>() / vols.len() as f64
    };
    (
        (avg.max(0.0).cbrt() * 100.0).round() as u32,
        p["mute"].as_bool().unwrap_or(false),
    )
}

fn text(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| v[*k].as_str().filter(|s| !s.is_empty()))
        .unwrap_or("")
        .to_string()
}

impl Graph {
    fn get(&self, id: u32) -> Option<&Object> {
        self.objects.iter().find(|o| o.id == id)
    }

    fn default_name(&self, key: &str) -> Option<String> {
        self.metadata.get(&(0, key.to_string())).and_then(named)
    }

    fn serial(o: &Object) -> u64 {
        o.props["object.serial"].as_u64().unwrap_or(0)
    }

    fn device_json(&self, o: &Object, default: Option<&str>) -> Value {
        let (vol, mute) = volume(o);
        let name = text(&o.props, &["node.name"]);
        json!({
            "id": o.id,
            "serial": Self::serial(o),
            "name": text(&o.props, &["node.description", "node.nick", "node.name"]),
            "volume": vol,
            "mute": mute,
            "default": default == Some(name.as_str()),
            "card": o.props["device.id"].as_u64(),
        })
    }

    fn stream_json(&self, o: &Object, playback: bool) -> Value {
        let (vol, mute) = volume(o);
        // A playback stream links into an output; a recording stream from an input.
        let through = self
            .links
            .iter()
            .find_map(|(a, b)| {
                if playback && *a == o.id {
                    Some(*b)
                } else if !playback && *b == o.id {
                    Some(*a)
                } else {
                    None
                }
            })
            .and_then(|n| self.get(n))
            .filter(|n| n.kind == "Node");
        let pinned = self
            .metadata
            .get(&(o.id, "target.object".to_string()))
            .is_some_and(|v| !v.is_null() && v != "-1");
        json!({
            "id": o.id,
            "app": text(&o.props, &["application.name", "node.description", "node.name"]),
            "title": text(&o.props, &["media.name"]),
            "icon": text(&o.props, &["application.icon-name"]),
            "volume": vol,
            "mute": mute,
            "through": through.map(|n| n.id),
            "through_serial": through.map(Self::serial),
            "pinned": pinned,
        })
    }

    fn card_json(&self, o: &Object) -> Option<Value> {
        let profiles = o.params["EnumProfile"].as_array()?;
        let current = o.params["Profile"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|p| p["index"].as_u64());
        let list: Vec<Value> = profiles
            .iter()
            .filter(|p| p["available"] != "no" || p["index"].as_u64() == current)
            .map(|p| {
                json!({
                    "index": p["index"],
                    "name": text(p, &["description", "name"]),
                })
            })
            .collect();
        if list.len() < 2 {
            return None;
        }
        Some(json!({
            "id": o.id,
            "name": text(&o.props, &["device.description", "device.nick", "device.name"]),
            "profiles": list,
            "profile": current,
        }))
    }

    /// Everything the page shows.
    pub fn state(&self) -> Value {
        let sink = self.default_name("default.audio.sink");
        let source = self.default_name("default.audio.source");
        let mut outputs = Vec::new();
        let mut inputs = Vec::new();
        let mut playback = Vec::new();
        let mut recording = Vec::new();
        let mut cards = Vec::new();
        for o in &self.objects {
            match (o.kind.as_str(), o.class.as_str()) {
                ("Node", "Audio/Sink") => outputs.push(self.device_json(o, sink.as_deref())),
                ("Node", "Audio/Source") => inputs.push(self.device_json(o, source.as_deref())),
                ("Node", "Stream/Output/Audio") => playback.push(self.stream_json(o, true)),
                ("Node", "Stream/Input/Audio") => recording.push(self.stream_json(o, false)),
                ("Device", "Audio/Device") => cards.extend(self.card_json(o)),
                _ => {}
            }
        }
        json!({
            "available": true,
            "outputs": outputs,
            "inputs": inputs,
            "playback": playback,
            "recording": recording,
            "cards": cards,
        })
    }

    fn is(&self, id: u32, classes: &[&str]) -> Option<&Object> {
        self.get(id).filter(|o| classes.contains(&o.class.as_str()))
    }
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
        // pw-dump prints the graph once, then each change; any line means "read again".
        let mut lines = self.tools.lines(argv(&["pw-dump", "--monitor"]));
        while lines.next().await.is_some() {
            changed.notify_one();
        }
        std::future::pending::<()>().await;
    }
}

fn path_arg(p: &Path) -> String {
    p.display().to_string()
}

#[cfg(test)]
mod tests;
