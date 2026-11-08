.pragma library

// Merged dock model: pinned apps in pin order, then running unpinned apps in first-seen order.
// toplevels: [{address, cls, title, workspace, minimised, monitor}]
// lookup(cls) -> desktop id | null; unknown classes become "class:<cls>".
function entries(pinned, toplevels, lookup) {
    var out = [], byId = {};
    function get(id, isPinned) {
        if (!byId[id]) {
            byId[id] = { id: id, pinned: isPinned, windows: [], visible: [], running: false, minimisedOnly: false };
            out.push(byId[id]);
        }
        return byId[id];
    }
    (pinned || []).forEach(function (id) { get(String(id), true); });
    (toplevels || []).forEach(function (t) {
        var cls = t && t.cls ? String(t.cls) : "";
        if (cls === "")
            return;
        var id = lookup(cls) || ("class:" + cls);
        var e = get(id, false);
        e.windows.push(t.address);
        if (!t.minimised)
            e.visible.push(t.address);
    });
    out.forEach(function (e) {
        e.running = e.windows.length > 0;
        e.minimisedOnly = e.running && e.visible.length === 0;
    });
    return out;
}

// The window after the focused one (focusOrder[0..] = most recently focused first), wrapping.
// Minimised windows are skipped unless every window is minimised.
function nextWindow(entry, focusOrder) {
    var list = entry.visible && entry.visible.length ? entry.visible : entry.windows;
    if (!list || list.length === 0)
        return null;
    for (var i = 0; i < (focusOrder || []).length; i++) {
        var idx = list.indexOf(focusOrder[i]);
        if (idx >= 0)
            return list[(idx + 1) % list.length];
    }
    return list[0];
}
