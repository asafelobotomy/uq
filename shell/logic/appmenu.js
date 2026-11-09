.pragma library
.import "launcher.js" as L
.import "power.js" as P

// App-menu logic: categories, favourites, what a power button does, and the avatar and hostname
// sources. Pure data in, data out; app names stay untouched for UqText to render, and nothing
// here runs anything.

// Shown in this order after Favourites and All apps, when they hold at least one app.
var CATEGORIES = ["Development", "Graphics", "Internet", "Multimedia", "Games", "Office",
                  "Settings", "System", "Utilities"];
var FAVOURITES = "Favourites";
var ALL = "All apps";

// freedesktop main category -> menu category. Anything else is Utilities.
var MAIN = {
    "Development": "Development",
    "Graphics": "Graphics",
    "Network": "Internet",
    "AudioVideo": "Multimedia",
    "Audio": "Multimedia",
    "Video": "Multimedia",
    "Game": "Games",
    "Office": "Office",
    "Education": "Office",
    "Settings": "Settings",
    "System": "System",
    "Utility": "Utilities"
};

function _list(v) {
    if (!v || typeof v === "string" || v.length === undefined)
        return [];
    return Array.prototype.slice.call(v);
}

// The menu category of an entry: its first freedesktop main category, in the entry's own order.
function category(entry) {
    var cats = _list(entry ? entry.categories : null);
    for (var i = 0; i < cats.length; i++) {
        var c = String(cats[i]);
        if (Object.prototype.hasOwnProperty.call(MAIN, c))
            return MAIN[c];
    }
    return "Utilities";
}

function _visible(entries) {
    return _list(entries).filter(function (e) { return e && !e.noDisplay; });
}

// [{key, count}]: Favourites (count = favouriteCount), All apps, then each non-empty category.
function categories(entries, favouriteCount) {
    var vis = _visible(entries);
    var counts = {};
    for (var i = 0; i < vis.length; i++) {
        var k = category(vis[i]);
        counts[k] = (counts[k] || 0) + 1;
    }
    var out = [{ key: FAVOURITES, count: Number(favouriteCount) || 0 },
               { key: ALL, count: vis.length }];
    for (var j = 0; j < CATEGORIES.length; j++)
        if (counts[CATEGORIES[j]])
            out.push({ key: CATEGORIES[j], count: counts[CATEGORIES[j]] });
    return out;
}

// The pinned entries in pin order; ids with no entry (or repeated) are skipped.
function favourites(pinned, entries) {
    var byId = {};
    var es = _list(entries);
    for (var i = 0; i < es.length; i++)
        if (es[i] && typeof es[i].id === "string")
            byId[es[i].id] = es[i];
    var out = [], seen = {};
    var ps = _list(pinned);
    for (var j = 0; j < ps.length; j++) {
        var id = ps[j];
        if (typeof id !== "string" || seen[id] || !Object.prototype.hasOwnProperty.call(byId, id))
            continue;
        seen[id] = true;
        out.push(byId[id]);
    }
    return out;
}

function _byName(a, b) {
    var an = String(a.name || "").toLowerCase(), bn = String(b.name || "").toLowerCase();
    return an < bn ? -1 : (an > bn ? 1 : 0);
}

// The grid for a category key: favourites in pin order, otherwise visible apps by name.
function appsIn(key, entries, pinned) {
    if (key === FAVOURITES)
        return favourites(pinned, entries);
    var vis = _visible(entries);
    if (key !== ALL) {
        if (CATEGORIES.indexOf(key) < 0)
            return [];
        vis = vis.filter(function (e) { return category(e) === key; });
    }
    return vis.sort(_byName);
}

// launcher.appEntries() copies plus each entry's categories, for category().
function menuEntries(list) {
    var raw = _list(list).filter(function (d) { return !!d; });
    var out = L.appEntries(raw);
    for (var i = 0; i < out.length; i++)
        out[i].categories = _list(raw[i].categories).map(String);
    return out;
}

// --- power ---
// powerClick(action) -> "lock" | "fire" | "powermenu" | ""
//   "lock":      close the app menu and call Session.lock(), as the power menu does.
//   "fire":      logout and suspend: send Power [action] now.
//   "powermenu": reboot and poweroff: close the app menu and open the power menu at that
//                action's confirm step (its 30 s countdown, power.js).
//   "":          anything else does nothing.
function powerClick(action) {
    if (action === "lock")
        return "lock";
    if (P.needsConfirm(action))
        return "powermenu";
    return P.ACTIONS.indexOf(action) >= 0 ? "fire" : "";
}

// --- header and footer ---

function _safeUser(user) {
    var u = String(user === undefined || user === null ? "" : user);
    return u !== "" && u.indexOf("/") < 0 && !/^\.+$/.test(u) ? u : "";
}

// Local avatar candidates in order: $HOME/.face, then AccountsService's icon for $USER, as
// file:// URLs encoded the way Spotlight encodes local icon paths. Only built from HOME and
// USER, never from other data.
function avatarSources(home, user) {
    var out = [];
    var h = String(home === undefined || home === null ? "" : home).replace(/\/+$/, "");
    if (h.charAt(0) === "/" && h.charAt(1) !== "/") {
        var r = L.iconRef(h + "/.face");
        if (r.path !== undefined)
            out.push(r.path);
    }
    var u = _safeUser(user);
    if (u !== "") {
        var r2 = L.iconRef("/var/lib/AccountsService/icons/" + u);
        if (r2.path !== undefined)
            out.push(r2.path);
    }
    return out;
}

// The fallback avatar letter.
function initial(user) {
    var chars = Array.from(String(user === undefined || user === null ? "" : user).trim());
    return chars.length > 0 ? chars[0].toUpperCase() : "?";
}

// $HOSTNAME if set, else the first line of /etc/hostname, trimmed; "localhost" if neither.
function hostname(envValue, fileText) {
    var e = String(envValue === undefined || envValue === null ? "" : envValue).trim();
    if (e !== "")
        return e;
    var f = String(fileText === undefined || fileText === null ? "" : fileText).split("\n")[0].trim();
    return f !== "" ? f : "localhost";
}
