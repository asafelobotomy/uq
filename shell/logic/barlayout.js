.pragma library

// The bar's layout (`shell.bar`): three zones of islands, each `{id, size, units}` where size is
// one of four tiers, in multiples of the Icon size (a square island as tall as the bar): icon 1x,
// small 2x, medium 4x, large 6x; or "custom", wider than large in whole Icon sizes (`units`,
// MIN_CUSTOM to MAX_CUSTOM); absent means the island's own default. Pure helpers, no
// Quickshell imports. uqd validates every write; these only keep the shell from sending what it
// would refuse, give an older uqd (no `bar` yet) the layout its switches meant, and decide how
// the islands fit a screen: never overlapping, never cut off, stepping islands down a size (and
// as a last resort leaving some out) instead.

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
    { id: "media", name: "Now playing" },
    { id: "stocks", name: "Stock ticker" },
    { id: "news", name: "News headlines" },
    { id: "sysmon", name: "System monitor" },
    { id: "worldclock", name: "World clock" },
    { id: "keyboard", name: "Keyboard layout" },
    { id: SEPARATOR, name: "Separator" }
];

// Left out first when even every island at Icon size doesn't fit: separators, then these, in
// this order. Same idea for which island steps down first among equals.
var EXPENDABLE = [SEPARATOR, "keyboard", "worldclock", "news", "stocks", "sysmon", "media",
                  "weather", "notif", "work", "clock", "system", "status"];

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
    case "clock": case "media": case "stocks": case "worldclock": return "medium";
    case "keyboard": return "icon";
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

// Copy of `item` with `extra` fields (only `text` when it is not the theme's look).
function _withText(item, out) {
    var t = _storedText(textOf(item));
    if (t !== null && item.id !== SEPARATOR)
        out.text = t;
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

// Move the item at zone/index to toIndex of toZone (counted after taking it out). Null when
// nothing would change or the place doesn't exist.
function moveTo(bar, zone, index, toZone, toIndex) {
    if (ZONES.indexOf(zone) < 0 || ZONES.indexOf(toZone) < 0 || index < 0
            || index >= bar[zone].length)
        return null;
    var out = _copy(bar);
    var item = out[zone].splice(index, 1)[0];
    var to = Math.max(0, Math.min(toIndex, out[toZone].length));
    if (zone === toZone && to === index)
        return null;
    out[toZone].splice(to, 0, item);
    return out;
}

// One step left (dir -1) or right (+1); past a zone's edge it joins the next zone at the near
// end. Null at the far left or right of the bar.
function move(bar, zone, index, dir) {
    var zi = ZONES.indexOf(zone);
    if (zi < 0 || index < 0 || index >= bar[zone].length)
        return null;
    var to = index + dir;
    if (to >= 0 && to < bar[zone].length)
        return moveTo(bar, zone, index, zone, to);
    var nz = zi + dir;
    if (nz < 0 || nz >= ZONES.length)
        return null;
    return moveTo(bar, zone, index, ZONES[nz], dir < 0 ? bar[ZONES[nz]].length : 0);
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
    list.splice(at, 0, { id: id, size: null });
    return out;
}

// Set a size (with `units` for Custom; missing, the item keeps its width or takes the
// narrowest); the island's own default is stored as "no size".
function setSize(bar, zone, index, size, units) {
    if (SIZES.indexOf(size) < 0 || ZONES.indexOf(zone) < 0 || index < 0
            || index >= bar[zone].length)
        return null;
    var out = _copy(bar);
    var old = out[zone][index], id = old.id;
    if (size === CUSTOM)
        out[zone][index] = _withText(old, { id: id, size: CUSTOM,
                             units: _cleanUnits(units !== undefined ? units : old.units) });
    else
        out[zone][index] = _withText(old, { id: id, size: size === defaultSize(id) ? null : size });
    return out;
}

// Set the text style of the item at zone/index: `t` is merged into what it has (so
// {bold: true} changes only that); null puts back the theme's look. Null for a separator or
// a place that doesn't exist.
function setText(bar, zone, index, t) {
    if (ZONES.indexOf(zone) < 0 || index < 0 || index >= bar[zone].length
            || bar[zone][index].id === SEPARATOR)
        return null;
    var out = _copy(bar);
    var old = out[zone][index], next = {};
    for (var k in old)
        if (k !== "text")
            next[k] = old[k];
    var cur = textOf(old);
    if (t !== null)
        for (var f in TEXT_DEFAULT)
            if (t[f] !== undefined)
                cur[f] = t[f];
    var stored = t === null ? null : _storedText(textOf({ text: cur }));
    if (stored !== null)
        next.text = stored;
    out[zone][index] = next;
    return out;
}

// The next size up (dir 1) or down (-1) from `size` (with `units` for Custom), as
// {size, units}, or null past the ends. Past Large it goes Custom, one Icon size at a time.
function stepSize(size, dir, units) {
    if (size === CUSTOM) {
        var u = _cleanUnits(units) + dir;
        if (u > MAX_CUSTOM)
            return null;
        return u < MIN_CUSTOM ? { size: "large", units: null } : { size: CUSTOM, units: u };
    }
    var i = SIZES.indexOf(size) + dir;
    if (i < 0 || i >= SIZES.length)
        return null;
    return SIZES[i] === CUSTOM ? { size: CUSTOM, units: MIN_CUSTOM } : { size: SIZES[i], units: null };
}

// The size nearest `px` wide, for a resize by dragging: {size, units}. Up to Large it snaps to
// the tiers; past Large it is Custom, in whole Icon sizes.
function sizeForWidth(px, unit) {
    var best = "icon", bestD = Infinity;
    for (var i = 0; i < SIZES.length; i++) {
        if (SIZES[i] === CUSTOM)
            continue;
        var d = Math.abs(UNITS[SIZES[i]] * unit - px);
        if (d < bestD) {
            bestD = d;
            best = SIZES[i];
        }
    }
    var n = Math.round(px / unit);
    if (n >= MIN_CUSTOM && Math.abs(n * unit - px) < bestD)
        return { size: CUSTOM, units: Math.min(n, MAX_CUSTOM) };
    return { size: best, units: null };
}

// Whether {size, units} is what `item` has now.
function sameSize(item, s) {
    return sizeOf(item) === s.size && (s.size !== CUSTOM || unitsOf(item) === s.units);
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

// What SetBarLayout takes: sizes only when set, options as they were.
function toSetting(bar) {
    var z = function (list) {
        return list.map(function (it) {
            if (it.size === CUSTOM)
                return _withText(it, { id: it.id, size: CUSTOM, units: _cleanUnits(it.units) });
            return _withText(it, SIZES.indexOf(it.size) >= 0 ? { id: it.id, size: it.size } : { id: it.id });
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

// --- fitting a screen ---

// metrics: {unit, gap (between islands), zoneGap (between zones), sep (a separator's width)}.
function _widthOf(id, units, m) {
    return id === SEPARATOR ? m.sep : units * m.unit;
}

function _rank(id) {
    var i = EXPENDABLE.indexOf(id);
    return i < 0 ? EXPENDABLE.length : i;
}

// How every island is drawn on a bar `width` wide: {sizes: {key: size}, units: {key: width in
// Icon sizes}, hidden: {key: true}, hiddenCount, width (what it all takes)}. While it doesn't
// fit, the biggest island steps down a size (a Custom one an Icon size at a time; the most
// expendable first among equals); once every island is at Icon size, the most expendable are
// left out. The result always fits (an empty bar fits anything).
function fit(bar, width, m) {
    var items = [];
    for (var z = 0; z < ZONES.length; z++) {
        var list = bar[ZONES[z]];
        for (var i = 0; i < list.length; i++)
            items.push({ k: key(ZONES[z], i), zone: ZONES[z], id: list[i].id,
                         size: sizeOf(list[i]), units: unitsOf(list[i]), hidden: false });
    }
    function total() {
        var sum = 0, per = { start: 0, center: 0, end: 0 }, zones = 0;
        for (var j = 0; j < items.length; j++) {
            if (items[j].hidden)
                continue;
            sum += _widthOf(items[j].id, items[j].units, m);
            per[items[j].zone]++;
        }
        for (var zz = 0; zz < ZONES.length; zz++)
            if (per[ZONES[zz]] > 0) {
                sum += (per[ZONES[zz]] - 1) * m.gap;
                zones++;
            }
        return sum + Math.max(0, zones - 1) * m.zoneGap;
    }
    var guard = 0;
    while (total() > width && guard++ < 2000) {
        var pick = null;
        for (var a = 0; a < items.length; a++) {
            var it = items[a];
            if (it.hidden || it.id === SEPARATOR || it.size === "icon")
                continue;
            if (pick === null || it.units > pick.units
                    || (it.units === pick.units && _rank(it.id) < _rank(pick.id)))
                pick = it;
        }
        if (pick !== null) {
            var next = stepSize(pick.size, -1, pick.units);
            pick.size = next.size;
            pick.units = next.size === CUSTOM ? next.units : UNITS[next.size];
            continue;
        }
        var drop = null;
        for (var b = items.length - 1; b >= 0; b--) {
            if (items[b].hidden)
                continue;
            if (drop === null || _rank(items[b].id) < _rank(drop.id))
                drop = items[b];
        }
        if (drop === null)
            break;
        drop.hidden = true;
    }
    var out = { sizes: {}, units: {}, hidden: {}, hiddenCount: 0, width: total() };
    for (var c = 0; c < items.length; c++) {
        out.sizes[items[c].k] = items[c].size;
        out.units[items[c].k] = items[c].units;
        if (items[c].hidden) {
            out.hidden[items[c].k] = true;
            out.hiddenCount++;
        }
    }
    return out;
}

// Where each shown island goes: {key: {x, w, size, units}}. The left zone from the left edge, the right
// zone from the right edge, the centre zone centred unless that would touch a neighbour, in
// which case it moves just enough. With a fitted layout nothing overlaps.
function place(bar, fitted, width, m) {
    var out = {};
    function row(zone) {
        var list = bar[zone], xs = [], x = 0;
        for (var i = 0; i < list.length; i++) {
            var k = key(zone, i);
            if (fitted.hidden[k])
                continue;
            var u = fitted.units && fitted.units[k] !== undefined ? fitted.units[k] : UNITS[fitted.sizes[k]];
            var w = _widthOf(list[i].id, u, m);
            if (xs.length > 0)
                x += m.gap;
            xs.push({ k: k, x: x, w: w, size: fitted.sizes[k], units: u });
            x += w;
        }
        return { items: xs, width: x };
    }
    var s = row("start"), c = row("center"), e = row("end");
    var endLeft = width - e.width;
    var lo = s.items.length > 0 ? s.width + m.zoneGap : 0;
    var hi = (e.items.length > 0 ? endLeft - m.zoneGap : width) - c.width;
    var cx = Math.max(lo, Math.min(Math.round((width - c.width) / 2), hi));
    var parts = [[s, 0], [c, cx], [e, endLeft]];
    for (var p = 0; p < parts.length; p++)
        for (var j = 0; j < parts[p][0].items.length; j++) {
            var it = parts[p][0].items[j];
            out[it.k] = { x: parts[p][1] + it.x, w: it.w, size: it.size, units: it.units };
        }
    return out;
}

// Places an island being dragged can be dropped: [{zone, index, x}], index counted in the zone
// as it is now. Between neighbours, at each zone's ends, and an empty zone's usual spot.
function dropPoints(bar, placedMap, width) {
    var pts = [];
    var anchors = { start: 0, center: width / 2, end: width };
    for (var z = 0; z < ZONES.length; z++) {
        var zone = ZONES[z], list = bar[zone], shown = [];
        for (var i = 0; i < list.length; i++) {
            var p = placedMap[key(zone, i)];
            if (p)
                shown.push({ i: i, x: p.x, w: p.w });
        }
        if (shown.length === 0) {
            pts.push({ zone: zone, index: list.length, x: anchors[zone] });
            continue;
        }
        for (var j = 0; j < shown.length; j++)
            pts.push({ zone: zone, index: shown[j].i, x: shown[j].x });
        var last = shown[shown.length - 1];
        pts.push({ zone: zone, index: last.i + 1, x: last.x + last.w });
    }
    return pts;
}

// The drop point nearest `x`.
function nearestDrop(points, x) {
    var best = null, bestD = Infinity;
    for (var i = 0; i < points.length; i++) {
        var d = Math.abs(points[i].x - x);
        if (d < bestD) {
            bestD = d;
            best = points[i];
        }
    }
    return best;
}

// The move edit for dropping the item at zone/index on drop point `pt` (whose index is counted
// before the item is taken out). Null when it stays where it is.
function dropEdit(bar, zone, index, pt) {
    if (pt === null)
        return null;
    var to = pt.index;
    if (pt.zone === zone && to > index)
        to--;
    return moveTo(bar, zone, index, pt.zone, to);
}

// Every shown island in bar order, as [{zone, index, key}], for walking them with the keyboard.
function order(bar, fitted) {
    var out = [];
    for (var z = 0; z < ZONES.length; z++)
        for (var i = 0; i < bar[ZONES[z]].length; i++) {
            var k = key(ZONES[z], i);
            if (!fitted || !fitted.hidden[k])
                out.push({ zone: ZONES[z], index: i, key: k });
        }
    return out;
}
