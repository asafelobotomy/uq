.pragma library

// Pure logic for widgets from other apps and app actions on launcher rows (Phase 4 spec §2.2,
// §2.4.2). No Quickshell imports; the bridge has already validated every manifest, so this only
// guards the shapes it sends and decides what shows where.

var BAR_MAX = 4;            // bar icons before the rest go under "…"
var RETRY_MS = 60000;       // a widget that failed is tried again at most once a minute
var ID_RE = /^[a-z0-9-]{1,40}$/;
var DESKTOP_RE = /^[A-Za-z0-9_][A-Za-z0-9._-]{0,199}\.desktop$/;

function _str(v, max) {
    return typeof v === "string" && v.length > 0 && v.length <= max ? v : null;
}

// The `Widgets` event data as a clean list, by order then id. Entries of the wrong shape are
// dropped; a widget with a URL other than a local file is dropped too.
function widgets(data) {
    if (!Array.isArray(data))
        return [];
    var out = [], seen = {};
    for (var i = 0; i < data.length; i++) {
        var w = data[i];
        if (!w || typeof w !== "object")
            continue;
        var id = _str(w.id, 40), name = _str(w.name, 100), url = _str(w.qml_url, 4096);
        if (id === null || !ID_RE.test(id) || seen[id] || name === null || url === null
                || url.indexOf("file:///") !== 0 || (w.slot !== "bar" && w.slot !== "quick"))
            continue;
        var order = typeof w.order === "number" && isFinite(w.order)
            ? Math.max(0, Math.min(100, Math.round(w.order))) : 50;
        seen[id] = true;
        out.push({ id: id, name: name, slot: w.slot, order: order, qmlUrl: url,
                   visible: w.visible === true });
    }
    out.sort(function (a, b) {
        return a.order !== b.order ? a.order - b.order : (a.id < b.id ? -1 : (a.id > b.id ? 1 : 0));
    });
    return out;
}

// The visible widgets of one slot ("bar" or "quick"), in order.
function inSlot(list, slot) {
    return (list || []).filter(function (w) { return w.slot === slot && w.visible; });
}

// Bar icons: the first `max` show in the island, the rest are listed under "…".
function barLayout(list, max) {
    var m = max === undefined ? BAR_MAX : max;
    var all = inSlot(list, "bar");
    return { shown: all.slice(0, m), overflow: all.slice(m) };
}

// The Apps group shows only when at least one quick tile loaded.
function appsGroupVisible(loadedCount) {
    return loadedCount > 0;
}

// --- load failures: unloaded, logged once per widget per session, retried at most once a minute ---

// A fresh registry: {id: {at: ms of the last failure, logged: bool}}.
function failures() {
    return {};
}

// Whether `id` may be loaded at `now` (ms).
function mayLoad(reg, id, now) {
    var f = reg ? reg[id] : undefined;
    return !f || now - f.at >= RETRY_MS;
}

// Record a failure of `id` at `now`; returns true when this is the first one (log it).
function noteFailure(reg, id, now) {
    var f = reg[id];
    var first = !f;
    reg[id] = { at: now, logged: true };
    return first;
}

// The delay (ms) before `id` may be tried again.
function retryIn(reg, id, now) {
    var f = reg ? reg[id] : undefined;
    return f ? Math.max(0, RETRY_MS - (now - f.at)) : 0;
}

// The props object after a `WidgetProps` update: a new object so bindings see the change.
function mergeProps(old, props) {
    var out = {};
    var k;
    if (old && typeof old === "object")
        for (k in old)
            out[k] = old[k];
    if (props && typeof props === "object" && !Array.isArray(props))
        for (k in props)
            out[k] = props[k];
    return out;
}

// --- app actions ---

// The `AppActions` event data as a clean list.
function appActions(data) {
    if (!Array.isArray(data))
        return [];
    var out = [];
    for (var i = 0; i < data.length; i++) {
        var a = data[i];
        if (!a || typeof a !== "object")
            continue;
        var id = _str(a.id, 40), name = _str(a.name, 100), glob = _str(a.match, 200);
        if (id === null || !ID_RE.test(id) || name === null || glob === null)
            continue;
        out.push({ id: id, name: name, icon: _str(a.icon, 64) || "", match: glob,
                   terminal: a.terminal === true });
    }
    return out;
}

// A launcher row's id as a desktop id: "firefox" -> "firefox.desktop"; null if it can't be one.
function desktopId(appId) {
    if (typeof appId !== "string" || appId === "")
        return null;
    var id = /\.desktop$/.test(appId) ? appId : appId + ".desktop";
    return DESKTOP_RE.test(id) ? id : null;
}

// `*` matches any run of characters, `?` one; everything else is literal (as the bridge).
function globMatch(glob, text) {
    var g = String(glob), t = String(text);
    var gi = 0, ti = 0, star = -1, mark = 0;
    while (ti < t.length) {
        if (gi < g.length && g[gi] === "*") {
            star = gi++;
            mark = ti;
        } else if (gi < g.length && (g[gi] === "?" || g[gi] === t[ti])) {
            gi++;
            ti++;
        } else if (star >= 0) {
            gi = star + 1;
            ti = ++mark;
        } else {
            return false;
        }
    }
    while (gi < g.length && g[gi] === "*")
        gi++;
    return gi === g.length;
}

// The actions for the row of `appId`, by name.
function actionsFor(actions, appId) {
    var id = desktopId(appId);
    if (id === null)
        return [];
    return (actions || []).filter(function (a) { return globMatch(a.match, id); })
        .sort(function (a, b) { return a.name < b.name ? -1 : (a.name > b.name ? 1 : 0); });
}

// The bridge call that runs `action` on the row of `appId`, or null. The bridge builds the
// command line; this names the action and the row only.
function actionCall(action, appId) {
    var id = desktopId(appId);
    if (!action || typeof action.id !== "string" || id === null)
        return null;
    return { call: "Launch", args: [{ app_action: action.id, desktop_id: id }] };
}
