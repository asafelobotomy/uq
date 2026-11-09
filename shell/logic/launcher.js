.pragma library

// Launcher logic shared by Spotlight and the app menu: query classification, the surface state
// machine, keyboard selection, and what Enter does with a row. Pure data in, data out; nothing
// here runs anything, and provider strings are passed through untouched for UqText to render.

var DEFAULT_ICON = "system-search";

// "calc" | "run" | "emoji" | "search", from the leading `=`, `>` or `:` (after trimming).
function classify(query) {
    var q = String(query === undefined || query === null ? "" : query).trim();
    switch (q.charAt(0)) {
    case "=": return "calc";
    case ">": return "run";
    case ":": return "emoji";
    default: return "search";
    }
}

function nextGeneration(g) {
    return (Number(g) || 0) + 1;
}

// --- surface state machine ---
// state: { open: "closed" | "spotlight" | "appmenu", wipe: bool }. `wipe` is true when the
// transition closed a surface (or locked), so that surface must clear its query and results.

var SURFACES = ["closed", "spotlight", "appmenu"];

function initialState() {
    return { open: "closed", wipe: false };
}

function transition(state, event) {
    var open = state && SURFACES.indexOf(state.open) >= 0 ? state.open : "closed";
    var target;
    switch (event) {
    case "toggleSpotlight": target = open === "spotlight" ? "closed" : "spotlight"; break;
    case "toggleAppMenu": target = open === "appmenu" ? "closed" : "appmenu"; break;
    case "close": target = "closed"; break;
    case "lock": return { open: "closed", wipe: true };
    default: return { open: open, wipe: false };
    }
    return { open: target, wipe: open !== "closed" && target !== open };
}

// --- selection ---
// sections: [{items: [...]}, ...] (see sections()); selection: {group, index}; {-1, -1} = none.

function _flat(groups) {
    var out = [];
    var gs = groups || [];
    for (var g = 0; g < gs.length; g++) {
        var n = gs[g] && gs[g].items ? gs[g].items.length : 0;
        for (var i = 0; i < n; i++)
            out.push({ group: g, index: i });
    }
    return out;
}

function _none() {
    return { group: -1, index: -1 };
}

function clampSelection(selection, groups) {
    var flat = _flat(groups);
    if (flat.length === 0)
        return _none();
    var s = selection || {};
    var g = Number(s.group), i = Number(s.index);
    if (!(g >= 0 && g < groups.length) || !groups[g].items || groups[g].items.length === 0)
        return flat[0];
    var n = groups[g].items.length;
    return { group: g, index: i >= 0 ? Math.min(i, n - 1) : 0 };
}

function _pos(flat, s) {
    for (var k = 0; k < flat.length; k++)
        if (flat[k].group === s.group && flat[k].index === s.index)
            return k;
    return 0;
}

// key: "up" | "down" (wrap over the whole list) | "tab" | "backtab" (first item of the next or
// previous non-empty group, wrapping).
function move(selection, groups, key) {
    var flat = _flat(groups);
    if (flat.length === 0)
        return _none();
    var g0 = selection ? Number(selection.group) : -1;
    if (!(g0 >= 0 && g0 < groups.length) || !groups[g0].items || groups[g0].items.length === 0)
        return flat[0];   // no valid selection: start at the top instead of moving
    var s = clampSelection(selection, groups);
    var p = _pos(flat, s);
    var n = flat.length;
    switch (key) {
    case "down": return flat[(p + 1) % n];
    case "up": return flat[(p - 1 + n) % n];
    case "tab":
    case "backtab": {
        var heads = [];
        for (var k = 0; k < n; k++)
            if (flat[k].index === 0)
                heads.push(flat[k]);
        var h = 0;
        for (var j = 0; j < heads.length; j++)
            if (heads[j].group === s.group)
                h = j;
        var step = key === "tab" ? 1 : -1;
        return heads[(h + step + heads.length) % heads.length];
    }
    default: return s;
    }
}

// Turns rank.merge() output into the rows Spotlight draws: a top-hit section (top: true), then
// the groups, with the top hit taken out of its own group and empty groups dropped.
function sections(merged) {
    var out = [];
    if (!merged)
        return out;
    var top = merged.topHit;
    if (top && top.item)
        out.push({ provider: top.provider, providerName: top.providerName, top: true,
                   items: [top.item] });
    var gs = merged.groups || [];
    for (var g = 0; g < gs.length; g++) {
        var items = gs[g].items || [];
        if (top && top.item && gs[g].provider === top.provider) {
            items = items.filter(function (it) {
                return it !== top.item && !(it && it.id === top.item.id);
            });
        }
        if (items.length > 0)
            out.push({ provider: gs[g].provider, providerName: gs[g].providerName, top: false,
                       items: items });
    }
    return out;
}

// Whether the shown rows are out of date for what's in the field: a debounced search is still
// pending, or the trimmed text differs from the query the rows came from. Activating a stale row
// would launch the previous query's app or run the previous `>` command.
function resultsStale(searchPending, query, searched) {
    var q = String(query === undefined || query === null ? "" : query).trim();
    var s = String(searched === undefined || searched === null ? "" : searched);
    return searchPending === true || q !== s;
}

// What Enter does with a row: { kind: "launch"|"copy"|"activate", call, args, close }.
// Only bridge calls; provider data is never executed. null when the row can't be activated.
function activation(provider, item) {
    var p = String(provider === undefined || provider === null ? "" : provider);
    if (p === "" || !item || typeof item.id !== "string" || item.id === "")
        return null;
    if (p === "apps")
        return { kind: "launch", call: "Launch", args: [{ desktop_id: item.id }], close: true };
    if (p === "run")
        return { kind: "launch", call: "Launch", args: [{ run_id: item.id }], close: true };
    if (p === "calc")
        return { kind: "copy", call: "Activate", args: ["calc", item.id, "copy"], close: false };
    var acts = item.actions;
    var action = acts && acts.length > 0 && typeof acts[0] === "string" ? acts[0] : "";
    return { kind: "activate", call: "Activate", args: [p, item.id, action], close: true };
}

// The text to put on the clipboard from calc's Activate reply ({"copy": ...}), or null.
function copyValue(reply) {
    if (!reply || typeof reply !== "object" || reply.copy === undefined || reply.copy === null)
        return null;
    return String(reply.copy);
}

// An icon is an icon-theme name or an absolute local path, never a URL.
// Returns {theme: name} (resolve with Quickshell.iconPath) or {path: "file://..."}.
function iconRef(icon) {
    if (typeof icon === "string") {
        if (/^[A-Za-z0-9._-]+$/.test(icon) && !/^\.+$/.test(icon))
            return { theme: icon };
        if (icon.charAt(0) === "/" && icon.charAt(1) !== "/")
            return { path: "file://" + icon.split("/").map(encodeURIComponent).join("/") };
    }
    return { theme: DEFAULT_ICON };
}

// The "No emoji provider installed" hint: a `:` query with text, the provider deadline has
// passed, and no external provider answered for this generation.
function showEmojiHint(query, gotExternal, deadlinePassed) {
    if (classify(query) !== "emoji")
        return false;
    var rest = String(query).trim().slice(1).trim();
    return rest !== "" && !!deadlinePassed && !gotExternal;
}

// Plain copies of DesktopEntry objects in the shape rank.searchApps expects.
function appEntries(list) {
    var out = [];
    var l = list || [];
    for (var i = 0; i < l.length; i++) {
        var d = l[i];
        if (!d)
            continue;
        var kw = d.keywords;
        var keywords = typeof kw === "string"
            ? kw.split(/[;,]/).map(function (k) { return k.trim(); })
                .filter(function (k) { return k !== ""; })
            : (kw && kw.length !== undefined ? Array.prototype.slice.call(kw) : []);
        out.push({ id: String(d.id || ""), name: String(d.name || ""),
                   genericName: String(d.genericName || ""), comment: String(d.comment || ""),
                   icon: String(d.icon || ""), keywords: keywords, noDisplay: !!d.noDisplay });
    }
    return out;
}
