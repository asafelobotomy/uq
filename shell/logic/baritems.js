.pragma library

// The bar's islands and their items: ids, names, sizes, text styles and fill, finding one, and
// reading a layout from `shell.bar`. barlayout.js keeps the edits and the fit, and re-exports these.

var ZONES = ["start", "center", "end"];
var SEPARATOR = "separator";
var MAX_ITEMS = 24;
var CUSTOM = "custom";
var SIZES = ["icon", "small", "medium", "large", CUSTOM];
// Same as uq_core::settings::bar::{MIN_CUSTOM, MAX_CUSTOM}.
var MIN_CUSTOM = 7;
var MAX_CUSTOM = 24;
// Custom's entry is its narrowest; the item's `units` say how wide it is.
var UNITS = { icon: 1, small: 2, medium: 4, large: 6, custom: MIN_CUSTOM };
var SIZE_NAMES = { icon: "Icon", small: "Small", medium: "Medium", large: "Large", custom: "Custom" };

// An island's text style (`text` on an item; same rules as uq_core::settings::bar::TextStyle):
// font (a family, MONO for the theme's monospaced font, null for the theme's), bold, italic,
// caps, and size in percent of the theme's text, TEXT_MIN to TEXT_MAX in steps of TEXT_STEP.
var MONO = "monospace";
var TEXT_MIN = 70;
var TEXT_MAX = 150;
var TEXT_STEP = 10;
var TEXT_DEFAULT = { font: null, bold: false, italic: false, caps: false, size: 100 };

// Same ids and order as uq_core::settings::bar::ISLANDS.
var ISLANDS = [
    { id: "system", name: "App menu, workspaces and dock" },
    { id: "work", name: "Open windows" },
    { id: "clock", name: "Clock and calendar" },
    { id: "weather", name: "Weather" },
    { id: "notif", name: "Notifications" },
    { id: "status", name: "Status (network, sound, battery)" },
    { id: "tray", name: "Tray icons of other apps" },
    { id: "media", name: "Now playing" },
    { id: "stocks", name: "Stock ticker" },
    { id: "news", name: "News headlines" },
    { id: "sysmon", name: "System monitor" },
    { id: "worldclock", name: "World clock" },
    { id: "keyboard", name: "Keyboard layout" },
    { id: "security", name: "Security (oysterAV)" },
    { id: "crumbs", name: "Storage cleanup (crumBS)" },
    { id: "clip", name: "Clipboard history (uqlip)" },
    { id: SEPARATOR, name: "Separator" }
];

// Left out first when even every island at Icon size doesn't fit: separators, then these, in
// this order. Same idea for which island steps down first among equals.
var EXPENDABLE = [SEPARATOR, "keyboard", "worldclock", "news", "stocks", "sysmon", "media",
                  "weather", "tray", "security", "crumbs", "clip", "notif", "work", "clock", "system", "status"];

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

// Same as uq_core::settings::bar::default_size.
function defaultSize(id) {
    switch (id) {
    case "system": case "work": case "status": case "news": return "large";
    case "clock": case "media": case "stocks": case "worldclock": case "tray": return "medium";
    case "keyboard": case "security": return "icon";
    default: return "small";
    }
}

// The size an item asks for (its own, else its island's default).
function sizeOf(item) {
    return item && SIZES.indexOf(item.size) >= 0 ? item.size : defaultSize(item ? item.id : "");
}

// An item's width in Icon sizes, before the bar steps it down to fit.
function unitsOf(item) {
    var s = sizeOf(item);
    return s === CUSTOM ? _cleanUnits(item.units) : UNITS[s];
}

// The size as islands read it: what they show at Large, they show at Custom (with more room).
function tierOf(size) {
    return size === CUSTOM ? "large" : size;
}

// A size's name; Custom says how wide.
function sizeLabel(size, units) {
    return size === CUSTOM ? "Custom (" + _cleanUnits(units) + "×)" : SIZE_NAMES[size];
}

function _cleanUnits(u) {
    var n = Math.round(Number(u));
    return isFinite(n) ? Math.max(MIN_CUSTOM, Math.min(MAX_CUSTOM, n)) : MIN_CUSTOM;
}

function _validFont(f) {
    return typeof f === "string" && f.trim() !== "" && f.length <= 64 && !/[\u0000-\u001f\u007f]/.test(f);
}

// A text style cleaned up (every field present), from whatever an item holds.
function textOf(item) {
    var t = item && item.text && typeof item.text === "object" ? item.text : {};
    var n = Math.round(Number(t.size) / TEXT_STEP) * TEXT_STEP;
    return {
        font: _validFont(t.font) ? t.font : null,
        bold: t.bold === true,
        italic: t.italic === true,
        caps: t.caps === true,
        size: isFinite(n) && n > 0 ? Math.max(TEXT_MIN, Math.min(TEXT_MAX, n)) : 100
    };
}

// The text style every island has that has none of its own (`options.all.text`), cleaned up.
function allText(bar) {
    var o = bar && bar.options && bar.options.all;
    return textOf({ text: o && typeof o === "object" ? o.text : null });
}

// The text style an island is drawn in: its own, else the all-islands text.
function effectiveText(bar, item) {
    return item && item.text && typeof item.text === "object" ? textOf(item) : allText(bar);
}

// The layout with the all-islands text changed to `t` (fields as setText; null for the
// theme's) and every other option kept.
function withAllText(bar, t) {
    var cur = allText(bar);
    if (t === null)
        cur = TEXT_DEFAULT;
    else
        for (var f in TEXT_DEFAULT)
            if (t[f] !== undefined)
                cur[f] = t[f];
    var stored = _storedText(textOf({ text: cur }));
    return withOptions(bar, "all", stored === null ? {} : { text: stored });
}

function isPlainText(t) {
    return !t || (t.font === null && !t.bold && !t.italic && !t.caps && t.size === 100);
}

// What is stored for style `t`: only the fields that differ from the theme's, null for none.
function _storedText(t) {
    if (isPlainText(t))
        return null;
    var out = {};
    if (t.font !== null) out.font = t.font;
    if (t.bold) out.bold = true;
    if (t.italic) out.italic = true;
    if (t.caps) out.caps = true;
    if (t.size !== 100) out.size = t.size;
    return out;
}

// The font family to draw style `t` in, given the theme's interface and monospaced fonts.
function fontFamily(t, ui, mono) {
    if (!t || t.font === null)
        return ui;
    return t.font === MONO ? mono : t.font;
}

// The next text size up (dir 1) or down (-1) from `size`, or null past the ends.
function stepTextSize(size, dir) {
    var n = (Number(size) || 100) + dir * TEXT_STEP;
    return n < TEXT_MIN || n > TEXT_MAX ? null : n;
}

// Copy of `item`'s own settings onto `out`: `text` when it is not the theme's look, and `fill`
// when it is not the island's default (see fillOf).
function _withText(item, out) {
    var t = _storedText(textOf(item));
    // `{}` is the theme's own look on an island, set on purpose (with an all-islands text it
    // is how an island opts out); no `text` at all takes the all-islands text.
    if (item.id !== SEPARATOR && (t !== null || (item.text && typeof item.text === "object")))
        out.text = t === null ? {} : t;
    if (item.id !== SEPARATOR && typeof item.fill === "boolean" && item.fill !== defaultFill(item.id))
        out.fill = item.fill;
    return out;
}

// Whether an island fills the bar's empty space beside its zone when its item doesn't say:
// only Open windows (same as uq_core::settings::bar::default_fill).
function defaultFill(id) {
    return id === "work";
}

// Whether `item` fills the empty space beside its zone (place()).
function fillOf(item) {
    if (!item || item.id === SEPARATOR)
        return false;
    return typeof item.fill === "boolean" ? item.fill : defaultFill(item.id);
}

// Turn filling on or off for the item at zone/index. Null for a separator or a place that
// doesn't exist.
function setFill(bar, zone, index, on) {
    if (ZONES.indexOf(zone) < 0 || index < 0 || index >= bar[zone].length
            || bar[zone][index].id === SEPARATOR)
        return null;
    var out = _copy(bar);
    var old = out[zone][index], next = {};
    for (var k in old)
        if (k !== "fill")
            next[k] = old[k];
    if (on !== defaultFill(old.id))
        next.fill = on === true;
    out[zone][index] = next;
    return out;
}

function _cleanSize(s) {
    if (s === "compact") return "small";
    if (s === "wide") return "large";
    return SIZES.indexOf(s) >= 0 ? s : null;
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
        var size = it.id === SEPARATOR ? null : _cleanSize(it.size);
        out.push(_withText(it, size === CUSTOM ? { id: it.id, size: size, units: _cleanUnits(it.units) }
                                               : { id: it.id, size: size }));
    }
    return out;
}

function _fromIslands(islands, weather) {
    var on = function (id) { return !islands || islands[id] !== false; };
    var start = [], end = [];
    if (on("system")) start.push({ id: "system", size: null });
    if (on("work")) start.push({ id: "work", size: null });
    if (weather) end.push({ id: "weather", size: null });
    if (on("time")) end.push({ id: "clock", size: null });
    if (on("notif")) end.push({ id: "notif", size: null });
    if (on("status")) end.push({ id: "status", size: null });
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

function key(zone, index) {
    return zone + ":" + index;
}

function placed(bar, id) {
    return find(bar, id) !== null;
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
