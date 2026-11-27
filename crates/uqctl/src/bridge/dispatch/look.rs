//! The window look (border, rounding, opacity, blur) read back from Hyprland in one batch.

use super::*;

/// The bridge's `WindowLook` call: how Hyprland draws windows right now, so the shell can draw
/// shapes that match them (the screen-edge indicator's window frame). `{border, rounding,
/// gaps_in, gaps_out: [top, right, bottom, left], active_border, inactive_border, layout}`, the
/// colours as `#AARRGGBB` (the first stop of a gradient), `layout` the default tiling layout
/// (dwindle, master, scrolling…). An option that can't be read is left out.
pub async fn window_look() -> Result<Value, String> {
    window_look_from(&HyprSocket::from_env()).await
}

/// The options `WindowLook` reads: its key, then the Hyprland option.
pub(crate) const LOOK_OPTIONS: [(&str, &str); 7] = [
    ("border", "general:border_size"),
    ("rounding", "decoration:rounding"),
    ("gaps_in", "general:gaps_in"),
    ("gaps_out", "general:gaps_out"),
    ("active_border", "general:col.active_border"),
    ("inactive_border", "general:col.inactive_border"),
    ("layout", "general:layout"),
];

/// Hyprland's separator between the replies of a `[[BATCH]]` request.
pub(crate) const BATCH_SEPARATOR: &str = "\n\n\n";

/// The replies of one `[[BATCH]]` request, one per request in it; `None` if the count is off
/// (an older Hyprland, or a reply that held the separator), so the caller asks one by one.
pub(crate) fn split_batch(reply: &str, n: usize) -> Option<Vec<&str>> {
    let parts: Vec<&str> = reply.split(BATCH_SEPARATOR).collect();
    (parts.len() == n).then_some(parts)
}

/// `WindowLook` against `hypr`: every option in one `[[BATCH]]` request (one connection, one
/// trip through Hyprland's loop instead of seven), falling back to one request per option.
pub(crate) async fn window_look_from(hypr: &HyprSocket) -> Result<Value, String> {
    let batch: Vec<String> = LOOK_OPTIONS
        .iter()
        .map(|(_, opt)| format!("j/getoption {opt}"))
        .collect();
    let batched = hypr.request(&format!("[[BATCH]]{}", batch.join(";"))).await;
    let replies: Vec<Option<String>> = match batched
        .as_deref()
        .ok()
        .and_then(|r| split_batch(r, batch.len()))
    {
        Some(parts) => parts.into_iter().map(|p| Some(p.to_string())).collect(),
        None if matches!(
            batched,
            Err(CompositorError::NotRunning | CompositorError::Timeout(_))
        ) =>
        {
            return Err("Hyprland is not running".into());
        }
        None => {
            let mut one_by_one = Vec::with_capacity(batch.len());
            for req in &batch {
                one_by_one.push(hypr.request(req).await.ok());
            }
            one_by_one
        }
    };
    let mut out = serde_json::Map::new();
    for ((key, _), reply) in LOOK_OPTIONS.iter().zip(replies) {
        if let Some(v) = reply.as_deref().and_then(parse_look_option) {
            out.insert((*key).into(), v);
        }
    }
    if out.is_empty() {
        return Err("Hyprland is not running".into());
    }
    Ok(Value::Object(out))
}

/// One `j/getoption` reply as `WindowLook` gives it: an int; a CSS box (`"4 4 4 4"`, one to four
/// values as in CSS) as `[top, right, bottom, left]`; a short word (a layout name); a gradient's
/// first colour as `#AARRGGBB`.
pub(crate) fn parse_look_option(reply: &str) -> Option<Value> {
    let v: Value = serde_json::from_str(reply).ok()?;
    if let Some(n) = v.get("int").and_then(Value::as_i64) {
        return Some(Value::from(n.clamp(0, 1000)));
    }
    if let Some(css) = v.get("css").and_then(Value::as_str) {
        let n: Vec<i64> = css
            .split_whitespace()
            .map(|p| p.parse::<i64>().ok().map(|x| x.clamp(0, 1000)))
            .collect::<Option<_>>()?;
        let b = match n.as_slice() {
            [a] => [*a, *a, *a, *a],
            [a, b] => [*a, *b, *a, *b],
            [a, b, c] => [*a, *b, *c, *b],
            [a, b, c, d] => [*a, *b, *c, *d],
            _ => return None,
        };
        return Some(Value::from(b.to_vec()));
    }
    if let Some(name) = v.get("str").and_then(Value::as_str) {
        // A layout name: short and plain.
        return (!name.is_empty()
            && name.len() <= 32
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
        .then(|| Value::from(name));
    }
    let g = v.get("gradient").and_then(Value::as_str)?;
    let first = g.split_whitespace().next()?;
    let hex = first.strip_prefix("0x").unwrap_or(first);
    (hex.len() == 8 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| Value::from(format!("#{}", hex.to_ascii_lowercase())))
}
