//! The PipeWire graph from `pw-dump`: devices, streams, volumes and defaults.

use super::*;

/// The parts of a dump the page uses.
#[derive(Debug, Default)]
pub struct Graph {
    pub(super) objects: Vec<Object>,
    /// `(output node, input node)` of each link.
    pub(super) links: Vec<(u32, u32)>,
    /// The `default` metadata: `(subject, key) → value`.
    pub(super) metadata: HashMap<(u32, String), Value>,
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
pub(super) fn named(v: &Value) -> Option<String> {
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
pub(super) fn volume(o: &Object) -> (u32, bool) {
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

impl Graph {
    pub(super) fn get(&self, id: u32) -> Option<&Object> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub(super) fn default_name(&self, key: &str) -> Option<String> {
        self.metadata.get(&(0, key.to_string())).and_then(named)
    }

    pub(super) fn serial(o: &Object) -> u64 {
        o.props["object.serial"].as_u64().unwrap_or(0)
    }

    pub(super) fn device_json(&self, o: &Object, default: Option<&str>) -> Value {
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

    pub(super) fn stream_json(&self, o: &Object, playback: bool) -> Value {
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

    pub(super) fn card_json(&self, o: &Object) -> Option<Value> {
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

    pub(super) fn is(&self, id: u32, classes: &[&str]) -> Option<&Object> {
        self.get(id).filter(|o| classes.contains(&o.class.as_str()))
    }
}
