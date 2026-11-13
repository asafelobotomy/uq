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

var MAX_PINS = 32;

// Unresolved apps ("class:<cls>") have no desktop id, so they can be neither pinned nor launched.
function isClassId(id) {
    return String(id).indexOf("class:") === 0;
}
function canPin(entry) {
    return !!entry && !isClassId(entry.id);
}
// The desktop id to hand to Launch, or null when the entry must not be launched.
function launchId(entry) {
    return canPin(entry) ? String(entry.id) : null;
}
// New pin list after toggling `id`, or null when the change must not be made (settings not
// loaded yet, unpinnable id, or the list is full). Result is de-duplicated.
function togglePin(pins, id, loaded) {
    if (!loaded || !Array.isArray(pins) || isClassId(id))
        return null;
    var out = [];
    pins.forEach(function (p) {
        if (out.indexOf(p) < 0)
            out.push(p);
    });
    var i = out.indexOf(id);
    if (i >= 0) {
        out.splice(i, 1);
        return out;
    }
    if (out.length >= MAX_PINS)
        return null;
    out.push(id);
    return out;
}

// The app menu island's apps: pinned apps with no window open anywhere, in pin order. An open
// app is on the Open windows island instead, and comes back here once all its windows close.
function launchers(entries) {
    return (entries || []).filter(function (e) { return e && e.pinned && !e.running; });
}
