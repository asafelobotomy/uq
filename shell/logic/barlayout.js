.pragma library

// The bar's layout (`shell.bar`): three zones of islands, each `{id, size}`. Pure helpers, no
// Quickshell imports. uqd validates every write; these only keep the shell from sending what
// it would refuse, and give an older uqd (no `bar` yet) the layout its island switches meant.

var ZONES = ["start", "center", "end"];
var SEPARATOR = "separator";
var MAX_ITEMS = 24;
var SIZES = ["compact", "normal", "wide"];

// Same ids and order as uq_core::settings::bar::ISLANDS.
var ISLANDS = [
    { id: "system", name: "System (volume, network, battery)" },
    { id: "work", name: "Workspaces and windows" },
    { id: "clock", name: "Clock and calendar" },
    { id: "weather", name: "Weather" },
    { id: "notif", name: "Notifications" },
    { id: "status", name: "Status icons" },
    { id: "media", name: "Now playing" },
    { id: "stocks", name: "Stock ticker" },
    { id: "news", name: "News headlines" },
    { id: "sysmon", name: "System monitor" },
    { id: "worldclock", name: "World clock" },
    { id: "keyboard", name: "Keyboard layout" },
    { id: SEPARATOR, name: "Separator" }
];

function known(id) {
    for (var i = 0; i < ISLANDS.length; i++)
        if (ISLANDS[i].id === id)
            return true;
    return false;
}

function name(id) {
    for (var i = 0; i < ISLANDS.length; i++)
        if (ISLANDS[i].id === id)
            return ISLANDS[i].name;
    return String(id);
}

function _items(list, seen, count) {
    var out = [];
    if (!list || typeof list.length !== "number")
        return out;
    for (var i = 0; i < list.length; i++) {
        var it = list[i];
        if (!it || typeof it.id !== "string" || !known(it.id) || count.n >= MAX_ITEMS)
            continue;
        if (it.id !== SEPARATOR && seen[it.id])
            continue;
        seen[it.id] = true;
        count.n++;
        out.push({ id: it.id, size: SIZES.indexOf(it.size) >= 0 ? it.size : "normal" });
    }
    return out;
}

function _fromIslands(islands, weather) {
    var on = function (id) { return !islands || islands[id] !== false; };
    var start = [], end = [];
    if (on("system")) start.push({ id: "system", size: "normal" });
    if (on("work")) start.push({ id: "work", size: "normal" });
    if (weather) end.push({ id: "weather", size: "normal" });
    if (on("time")) end.push({ id: "clock", size: "normal" });
    if (on("notif")) end.push({ id: "notif", size: "normal" });
    if (on("status")) end.push({ id: "status", size: "normal" });
    return { start: start, center: [], end: end, options: {} };
}

// The layout to draw, from Settings.values: `shell.bar` cleaned up, or the old switches'.
function layout(values) {
    var shell = values && values.shell ? values.shell : null;
    var bar = shell && shell.bar && typeof shell.bar === "object" ? shell.bar : null;
    if (bar === null)
        return _fromIslands(shell ? shell.islands : null, !!(values && values.weather_location));
    var seen = {}, count = { n: 0 };
    return {
        start: _items(bar.start, seen, count),
        center: _items(bar.center, seen, count),
        end: _items(bar.end, seen, count),
        options: bar.options && typeof bar.options === "object" ? bar.options : {}
    };
}

function _copy(bar) {
    return {
        start: bar.start.slice(), center: bar.center.slice(), end: bar.end.slice(),
        options: bar.options
    };
}

function placed(bar, id) {
    for (var z = 0; z < ZONES.length; z++) {
        var list = bar[ZONES[z]];
        for (var i = 0; i < list.length; i++)
            if (list[i].id === id)
                return true;
    }
    return false;
}

function count(bar) {
    return bar.start.length + bar.center.length + bar.end.length;
}

// Islands that can still be added (every separator can).
function addable(bar) {
    var out = [];
    if (count(bar) >= MAX_ITEMS)
        return out;
    for (var i = 0; i < ISLANDS.length; i++)
        if (ISLANDS[i].id === SEPARATOR || !placed(bar, ISLANDS[i].id))
            out.push(ISLANDS[i]);
    return out;
}

// Move one step left (dir -1) or right (+1); past a zone's edge it joins the next zone at the
// near end. Null when it can't move (the far left or right of the bar).
function move(bar, zone, index, dir) {
    var zi = ZONES.indexOf(zone);
    if (zi < 0 || index < 0 || index >= bar[zone].length)
        return null;
    var out = _copy(bar);
    var list = out[zone];
    var to = index + dir;
    if (to >= 0 && to < list.length) {
        var it = list.splice(index, 1)[0];
        list.splice(to, 0, it);
        return out;
    }
    var nz = zi + dir;
    if (nz < 0 || nz >= ZONES.length)
        return null;
    var item = list.splice(index, 1)[0];
    var next = out[ZONES[nz]];
    if (dir < 0)
        next.push(item);
    else
        next.unshift(item);
    return out;
}

function canMove(bar, zone, index, dir) {
    return move(bar, zone, index, dir) !== null;
}

function remove(bar, zone, index) {
    if (ZONES.indexOf(zone) < 0 || index < 0 || index >= bar[zone].length)
        return null;
    var out = _copy(bar);
    out[zone].splice(index, 1);
    return out;
}

// Add island `id` at `index` of `zone` (the end when index is past it or missing).
function add(bar, id, zone, index) {
    if (!known(id) || ZONES.indexOf(zone) < 0 || count(bar) >= MAX_ITEMS
            || (id !== SEPARATOR && placed(bar, id)))
        return null;
    var out = _copy(bar);
    var list = out[zone];
    var at = typeof index === "number" && index >= 0 && index <= list.length ? index : list.length;
    list.splice(at, 0, { id: id, size: "normal" });
    return out;
}

function setSize(bar, zone, index, size) {
    if (SIZES.indexOf(size) < 0 || ZONES.indexOf(zone) < 0 || index < 0
            || index >= bar[zone].length)
        return null;
    var out = _copy(bar);
    out[zone][index] = { id: out[zone][index].id, size: size };
    return out;
}

// Where island `id` sits: {zone, index}, or null.
function find(bar, id) {
    for (var z = 0; z < ZONES.length; z++) {
        var list = bar[ZONES[z]];
        for (var i = 0; i < list.length; i++)
            if (list[i].id === id)
                return { zone: ZONES[z], index: i };
    }
    return null;
}

// What SetBarLayout takes: sizes only when not normal, options as they were.
function toSetting(bar) {
    var z = function (list) {
        return list.map(function (it) {
            return it.size && it.size !== "normal" ? { id: it.id, size: it.size } : { id: it.id };
        });
    };
    var out = { start: z(bar.start), center: z(bar.center), end: z(bar.end) };
    if (bar.options && Object.keys(bar.options).length > 0)
        out.options = bar.options;
    return out;
}

// A copy of the layout with options.<island> replaced.
function withOptions(bar, island, opts) {
    var out = _copy(bar);
    var o = {};
    for (var k in bar.options)
        o[k] = bar.options[k];
    o[island] = opts;
    out.options = o;
    return out;
}

// Width in pixels for a text island of `size`, as a multiple of the theme's font size.
function textWidth(size, fontSize) {
    var em = fontSize > 0 ? fontSize : 11;
    return Math.round(em * (size === "compact" ? 12 : (size === "wide" ? 32 : 20)));
}
