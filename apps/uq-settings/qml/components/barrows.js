.pragma library

// The bar layout editor's rows: the three zones flattened into one list with a heading row
// before each zone, so one list can be reordered by dragging or with the keyboard, and moving a
// row past a heading puts it in the next zone. Pure: no Kirigami, so tests load it directly.

var ZONES = ["start", "center", "end"];
var ZONE_NAMES = { start: "Left", center: "Centre", end: "Right" };

// Islands with an options panel under their row.
var WITH_OPTIONS = ["clock", "stocks", "news", "worldclock"];

function _zone(bar, z) {
    var l = bar && bar[z];
    return Array.isArray(l) ? l : [];
}

// [{kind: "zone", zone} | {kind: "item", zone, index, id, size}] in bar order.
function rows(bar) {
    var out = [];
    for (var i = 0; i < ZONES.length; i++) {
        var z = ZONES[i];
        out.push({ kind: "zone", zone: z, name: ZONE_NAMES[z] });
        var list = _zone(bar, z);
        for (var j = 0; j < list.length; j++) {
            var it = list[j];
            if (!it || typeof it.id !== "string")
                continue;
            out.push({ kind: "item", zone: z, index: j, id: it.id,
                       size: typeof it.size === "string" ? it.size : "normal" });
        }
    }
    return out;
}

// The move edit for dragging row `from` so it lands at row position `to` (an index into the
// list as it is now; the row then sits just before what is at `to`, or last when `to` is past
// the end). Null when it doesn't move or `from` isn't an island.
function moveEdit(list, from, to) {
    if (from < 0 || from >= list.length || list[from].kind !== "item")
        return null;
    var item = list[from];
    var rest = list.slice(0, from).concat(list.slice(from + 1));
    var at = to > from ? to - 1 : to;
    at = Math.max(1, Math.min(at, rest.length));   // never above the first heading
    var zone = "start", index = 0;
    for (var i = 0; i < at; i++) {
        if (rest[i].kind === "zone") {
            zone = rest[i].zone;
            index = 0;
        } else {
            index++;
        }
    }
    if (zone === item.zone && index === item.index)
        return null;
    return { op: "move", id: item.id, zone: item.zone, index: item.index, to_zone: zone, to_index: index };
}

// One step up (-1) or down (+1) in the list, crossing headings into the next zone.
function stepEdit(list, from, dir) {
    if (dir < 0)
        return from - 1 >= 1 ? moveEdit(list, from, from - 1) : null;
    return from + 1 < list.length ? moveEdit(list, from, from + 2) : null;
}

function removeEdit(row) {
    return { op: "remove", id: row.id, zone: row.zone, index: row.index };
}

function sizeEdit(row, size) {
    return { op: "size", id: row.id, zone: row.zone, index: row.index, size: size };
}

function addEdit(id, zone) {
    return { op: "add", id: id, zone: ZONES.indexOf(zone) >= 0 ? zone : "end" };
}

function placed(bar, id) {
    for (var i = 0; i < ZONES.length; i++) {
        var list = _zone(bar, ZONES[i]);
        for (var j = 0; j < list.length; j++)
            if (list[j] && list[j].id === id)
                return true;
    }
    return false;
}

// The islands from `catalog` ([{id, name}]) that can still be added; separators always can.
function addable(bar, catalog) {
    var out = [];
    for (var i = 0; i < catalog.length; i++)
        if (catalog[i].id === "separator" || !placed(bar, catalog[i].id))
            out.push(catalog[i]);
    return out;
}

function name(catalog, id) {
    for (var i = 0; i < catalog.length; i++)
        if (catalog[i].id === id)
            return catalog[i].name;
    return id;
}

// List edits for an option list (symbols, feeds, zones): returns the new list, or null.
function listAdd(list, value, max) {
    if (value === null || value === "" || list.indexOf(value) >= 0 || list.length >= max)
        return null;
    return list.concat([value]);
}

function listMove(list, i, dir) {
    var j = i + dir;
    if (i < 0 || i >= list.length || j < 0 || j >= list.length)
        return null;
    var out = list.slice();
    var v = out.splice(i, 1)[0];
    out.splice(j, 0, v);
    return out;
}

function listRemove(list, i) {
    if (i < 0 || i >= list.length)
        return null;
    var out = list.slice();
    out.splice(i, 1);
    return out;
}

// Same rules as uqd (uq_core::settings::bar); uqd has the last word.
function cleanSymbol(s) {
    var t = String(s).trim().toUpperCase();
    return /^[A-Z0-9^.=\-]{1,16}$/.test(t) && /[A-Z0-9]/.test(t) ? t : null;
}

function cleanFeed(u) {
    var s = String(u).trim();
    return s.length <= 512 && /^https:\/\/[A-Za-z0-9.\-:]+(\/|\?|#|$)/.test(s) && !/\s/.test(s) ? s : null;
}

function cleanZone(z) {
    var s = String(z).trim().replace(/ /g, "_");
    return /^(UTC|[A-Za-z][A-Za-z0-9_+\-]*\/[A-Za-z][A-Za-z0-9_+\-]*(\/[A-Za-z][A-Za-z0-9_+\-]*)?)$/.test(s) ? s : null;
}

var PRESET_FEEDS = [
    { name: "BBC News – World", url: "https://feeds.bbci.co.uk/news/world/rss.xml" },
    { name: "The Guardian – World", url: "https://www.theguardian.com/world/rss" },
    { name: "NPR News", url: "https://feeds.npr.org/1001/rss.xml" },
    { name: "Al Jazeera", url: "https://www.aljazeera.com/xml/rss/all.xml" },
    { name: "Hacker News", url: "https://hnrss.org/frontpage" },
    { name: "Ars Technica", url: "https://feeds.arstechnica.com/arstechnica/index" }
];
