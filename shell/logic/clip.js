.pragma library

// The clipboard history (uqlip) as the popup and the island use it. Pure: no Qt calls.

// `uqctl clip list --json`: an array of entries, pinned first then newest first. Returns the
// entries that are well formed as { id, kind, bytes, created (unix s), pinned, preview, path };
// null when the text isn't such a list.
function parseList(text) {
    var d;
    try {
        d = JSON.parse(text);
    } catch (e) {
        return null;
    }
    if (!Array.isArray(d))
        return null;
    var out = [];
    d.forEach(function (e) {
        if (!e || typeof e.id !== "number" || (e.kind !== "text" && e.kind !== "image"))
            return;
        out.push({
            id: e.id,
            kind: e.kind,
            bytes: typeof e.bytes === "number" ? e.bytes : 0,
            created: typeof e.created === "number" ? e.created : 0,
            pinned: e.pinned === true,
            preview: typeof e.preview === "string" ? e.preview : "",
            path: e.kind === "image" && typeof e.path === "string" ? e.path : ""
        });
    });
    return out;
}

// A line of `uqctl clip watch`: { paused, enabled, count }; null when it isn't a state line.
function parseState(line) {
    var d;
    try {
        d = JSON.parse(line);
    } catch (e) {
        return null;
    }
    if (!d || typeof d !== "object" || d.event !== "state")
        return null;
    return {
        paused: d.paused === true,
        enabled: d.enabled !== false,
        count: typeof d.count === "number" ? d.count : 0
    };
}

// Entries whose text contains `query` (any case); all of them for a blank query.
function filter(entries, query) {
    var q = String(query || "").trim().toLowerCase();
    if (q === "")
        return entries;
    return entries.filter(function (e) { return e.preview.toLowerCase().indexOf(q) >= 0; });
}

// One line for a row: whitespace collapsed, cut at `max` characters. Images: what they are.
function label(e, max) {
    if (e.kind === "image")
        return "Image  " + sizeText(e.bytes);
    var t = e.preview.replace(/\s+/g, " ").trim();
    var m = max || 120;
    return t.length > m ? t.slice(0, m - 1) + "…" : t;
}

function sizeText(n) {
    if (n < 1024)
        return n + " B";
    if (n < 1024 * 1024)
        return (n / 1024).toFixed(0) + " KB";
    return (n / 1024 / 1024).toFixed(1) + " MB";
}

// "now", "5m", "3h", "2d" since `created` (unix seconds), at `now` (unix seconds).
function age(created, now) {
    var s = Math.max(0, Math.floor(now - created));
    if (s < 60)
        return "now";
    if (s < 3600)
        return Math.floor(s / 60) + "m";
    if (s < 86400)
        return Math.floor(s / 3600) + "h";
    return Math.floor(s / 86400) + "d";
}

// The highlighted row after moving by `delta`, kept inside 0..count-1 (-1 when there are none).
function move(index, delta, count) {
    if (count <= 0)
        return -1;
    return Math.max(0, Math.min(count - 1, index + delta));
}

// Island tooltip/accessible text.
function summary(state) {
    if (!state || !state.enabled)
        return "Clipboard history is off";
    if (state.paused)
        return "Clipboard history is paused";
    return state.count === 1 ? "1 item in the clipboard history" : state.count + " items in the clipboard history";
}

// How long to wait before starting `uqctl clip watch` again after `misses` watches in a row that
// ended without a snapshot (uqlip not running): 5, 10, 30 s, then every 60 s.
function retryDelayMs(misses) {
    var steps = [5000, 10000, 30000, 60000];
    return steps[Math.max(0, Math.min(steps.length - 1, misses | 0))];
}
