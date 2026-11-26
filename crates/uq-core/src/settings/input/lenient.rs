//! Loading input settings leniently: a bad device or gesture costs only itself.

use super::*;

/// Entries load one by one: a bad one is dropped, the others are kept.
pub(crate) fn lenient_devices<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, DeviceOverride>, D::Error> {
    let Value::Object(obj) = Value::deserialize(d)? else {
        return Ok(BTreeMap::new());
    };
    Ok(obj
        .into_iter()
        .filter_map(|(name, v)| {
            let key = format!("input.devices.{name}");
            let e = parse_lenient(&key, &v, &DeviceOverride::FIELDS);
            if e.is_none() {
                tracing::warn!("[STATE-002] state: dropping input device entry {name:?} = {v}");
            }
            e.map(|e| (name, e))
        })
        .collect())
}

/// Hyprland turns spaces in a device name into `-`; so do we, on write.
pub(crate) fn device_key(name: &str) -> String {
    name.replace(' ', "-")
}

/// Gestures load entry by entry: a bad entry is dropped with a log line. Absent, `null` or
/// not a list is `None`, which means uQ's own two gestures.
pub(crate) fn lenient_gestures<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Vec<Gesture>>, D::Error> {
    let Value::Array(items) = Value::deserialize(d)? else {
        return Ok(None);
    };
    Ok(Some(
        items
            .into_iter()
            .enumerate()
            .filter_map(|(i, v)| {
                let g = serde_json::from_value::<Gesture>(v.clone())
                    .map_err(|e| e.to_string())
                    .and_then(|g| {
                        g.validate(&format!("input.gestures[{i}]"))
                            .map(|()| g)
                            .map_err(|e| e.to_string())
                    });
                if let Err(e) = &g {
                    tracing::warn!("[STATE-002] state: dropping gesture {v}: {e}");
                }
                g.ok()
            })
            .collect(),
    ))
}

/// Drop the gestures a write would refuse: bad ones, the later of two that clash, and any past
/// the limit.
pub(crate) fn repaired_gestures(
    list: Vec<Gesture>,
    errors: &mut Vec<SettingError>,
) -> Vec<Gesture> {
    let mut kept: Vec<Gesture> = Vec::new();
    for (i, g) in list.into_iter().enumerate() {
        let key = format!("input.gestures[{i}]");
        if let Err(e) = g.validate(&key) {
            errors.push(e);
            continue;
        }
        if kept.len() >= MAX_GESTURES {
            errors.push(invalid(&key, "dropped: more than 24 gestures"));
            continue;
        }
        kept.push(g);
        if let Some(c) = gestures::clashes_of(&kept).into_iter().next() {
            tracing::warn!(
                "[STATE-002] state: dropping a gesture: {}",
                crate::keybinds::conflict::message(&c)
            );
            errors.push(invalid(
                &format!("{key}.direction"),
                "dropped: it clashes with an earlier gesture",
            ));
            kept.pop();
        }
    }
    kept
}
