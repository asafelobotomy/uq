.pragma library

// What look=see>do_ shows before anything is typed: the apps just opened (Recents) and five
// pinned squares the person fills. Pure data in, data out. The lists are saved by LauncherState
// as plain desktop ids, so anything read back is checked here before the shell trusts it.

var SLOTS = 5;
var MAX_RECENTS = 8;
var SHOWN_RECENTS = 5;
var MAX_ID = 256;

function validId(id) {
    return typeof id === "string" && id !== "" && id.length <= MAX_ID
        && id.indexOf("/") < 0 && id.indexOf("\0") < 0 && id.charAt(0) !== "-"
        && id.slice(-8) === ".desktop";
}

// Exactly SLOTS entries: a desktop id, or "" for an empty square. Bad or repeated ids are empty.
function normalisePins(raw) {
    var out = [], seen = {};
    var list = Array.isArray(raw) ? raw : [];
    for (var i = 0; i < SLOTS; i++) {
        var id = list[i];
        if (validId(id) && !seen[id]) {
            seen[id] = true;
            out.push(id);
        } else {
            out.push("");
        }
    }
    return out;
}

// `id` into square `slot`; an app already in another square moves, leaving that one empty.
// "" empties the square. Out-of-range slots change nothing.
function setPin(pins, slot, id) {
    var out = normalisePins(pins);
    var s = Number(slot);
    if (!(s >= 0 && s < SLOTS) || Math.floor(s) !== s)
        return out;
    if (id === "" || id === null || id === undefined) {
        out[s] = "";
        return out;
    }
    if (!validId(id))
        return out;
    for (var i = 0; i < SLOTS; i++)
        if (out[i] === id)
            out[i] = "";
    out[s] = id;
    return out;
}

// Most recent first, no repeats, at most MAX_RECENTS.
function normaliseRecents(raw) {
    var out = [], seen = {};
    var list = Array.isArray(raw) ? raw : [];
    for (var i = 0; i < list.length && out.length < MAX_RECENTS; i++)
        if (validId(list[i]) && !seen[list[i]]) {
            seen[list[i]] = true;
            out.push(list[i]);
        }
    return out;
}

function pushRecent(recents, id) {
    if (!validId(id))
        return normaliseRecents(recents);
    return normaliseRecents([id].concat(normaliseRecents(recents).filter(function (r) { return r !== id; })));
}

function _byId(entries) {
    var m = {};
    var es = entries || [];
    for (var i = 0; i < es.length; i++)
        if (es[i] && typeof es[i].id === "string" && !es[i].noDisplay)
            m[es[i].id] = es[i];
    return m;
}

// The recent apps that are still installed, newest first, at most SHOWN_RECENTS.
function recentEntries(recents, entries) {
    var by = _byId(entries), out = [];
    var rs = normaliseRecents(recents);
    for (var i = 0; i < rs.length && out.length < SHOWN_RECENTS; i++)
        if (Object.prototype.hasOwnProperty.call(by, rs[i]))
            out.push(by[rs[i]]);
    return out;
}

// One object per square: { slot, id, entry } where entry is null for an empty square or an app
// that is no longer installed (the square then reads as empty, but keeps its id until changed).
function squares(pins, entries) {
    var by = _byId(entries), ps = normalisePins(pins), out = [];
    for (var i = 0; i < SLOTS; i++)
        out.push({ slot: i, id: ps[i],
                   entry: ps[i] !== "" && Object.prototype.hasOwnProperty.call(by, ps[i]) ? by[ps[i]] : null });
    return out;
}

// The picker's rows: apps matching nothing yet, by name (the first `limit`).
function pickList(entries, limit) {
    var es = [];
    var list = entries || [];
    for (var i = 0; i < list.length; i++)
        if (list[i] && !list[i].noDisplay && validId(list[i].id))
            es.push(list[i]);
    es.sort(function (a, b) {
        var x = String(a.name).toLowerCase(), y = String(b.name).toLowerCase();
        return x < y ? -1 : (x > y ? 1 : 0);
    });
    return es.slice(0, limit > 0 ? limit : 8);
}

function serialise(pins, recents) {
    return JSON.stringify({ v: 1, pins: normalisePins(pins), recents: normaliseRecents(recents) });
}

// { pins, recents } from saved text; anything unreadable gives empty lists.
function parse(text) {
    var o = null;
    try {
        o = JSON.parse(String(text));
    } catch (e) {
        o = null;
    }
    if (!o || typeof o !== "object")
        o = {};
    return { pins: normalisePins(o.pins), recents: normaliseRecents(o.recents) };
}
