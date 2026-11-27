.pragma library

// The Open windows island's order and attention: Hyprland addresses, sort modes and close
// actions, sorting the tabs (screen order, minimised last), and the windows that want attention.
// work.js re-exports these.

// Normalise a Hyprland address to 0x… and validate it against the bridge's pattern.
function normAddr(a) {
    var s = String(a);
    if (s.indexOf("0x") !== 0)
        s = "0x" + s;
    return /^0x[0-9a-f]{1,16}$/.test(s) ? s : null;
}

// "screen" (the default) is the order the windows sit in on screen: left to right, as the
// scrolling layout's columns run.
var SORTS = ["screen", "opened", "activity", "pin", "usage", "name"];
var DEFAULT_SORT = "screen";
var SORT_NAMES = {
    screen: "Screen order", opened: "Last opened", activity: "Last activity", pin: "By pin",
    usage: "By system usage", name: "By name"
};
// The two directions, in words, for the modes that have them (not "pin").
var ORDER_NAMES = {
    opened: ["Oldest first", "Newest first"],
    activity: ["Most recent first", "Least recent first"],
    usage: ["Busiest first", "Least busy first"],
    name: ["A to Z", "Z to A"]
};

function sortHasOrder(mode) {
    return ORDER_NAMES[mode] !== undefined;
}

// What a tab's X does to an open window (shell.bar.options.work.close): ask first ("Minimize to
// Tab?"), minimise it to its tab, or close it.
var CLOSES = ["ask", "minimize", "close"];
var CLOSE_NAMES = { ask: "Ask each time", minimize: "Minimise to the tab", close: "Close the window" };

// What pressing a tab's X does now: "close", "minimise" or "ask". A minimised window's X closes it.
function closeAction(mode, minimised) {
    if (minimised === true)
        return "close";
    if (mode === "minimize")
        return "minimise";
    if (mode === "close")
        return "close";
    return "ask";
}

// The windows in the island's order. ctx: {opened: {addr: n} (lower opened earlier), activity:
// {addr: ms} (the last notification from its app), pinIndex: {addr: i} (absent = not pinned),
// usage: {addr: score}, name: {addr: "app name"}}. `descending` reverses the natural order
// (oldest, most recent, busiest, A first); "pin" has one order. Ties keep opening order.
function sortWindows(list, mode, descending, ctx) {
    var c = ctx || {};
    function num(map, a, dflt) {
        var v = map && typeof map[a] === "number" ? map[a] : dflt;
        return v;
    }
    var opened = function (a) { return num(c.opened, a, Number.MAX_SAFE_INTEGER); };
    var keyed = list.map(function (w, i) { return { w: w, i: i }; });
    var rev = descending === true && sortHasOrder(mode) ? -1 : 1;
    keyed.sort(function (x, y) {
        var a = x.w.address, b = y.w.address, d = 0;
        switch (mode) {
        case "activity":
            d = num(c.activity, b, 0) - num(c.activity, a, 0);
            break;
        case "pin":
            d = num(c.pinIndex, a, 1e6) - num(c.pinIndex, b, 1e6);
            break;
        case "usage":
            d = num(c.usage, b, 0) - num(c.usage, a, 0);
            break;
        case "name":
            var na = String((c.name && c.name[a]) || x.w.cls || "").toLowerCase();
            var nb = String((c.name && c.name[b]) || y.w.cls || "").toLowerCase();
            d = na < nb ? -1 : (na > nb ? 1 : 0);
            if (d === 0) {
                var ta = x.w.title.toLowerCase(), tb = y.w.title.toLowerCase();
                d = ta < tb ? -1 : (ta > tb ? 1 : 0);
            }
            break;
        case "screen":
            return 0; // screenOrder() places these
        default:
            d = opened(a) - opened(b);
        }
        if (d !== 0)
            return d * rev;
        var o = opened(a) - opened(b);
        return o !== 0 ? o : x.i - y.i;
    });
    var out = keyed.map(function (k) { return k.w; });
    return minimisedLast(mode === "screen" ? screenOrder(out, c.lastOrder) : out, c.lastMin);
}

// Minimised windows go to the end of the queue, whatever the order: the live windows keep the
// order they had, then the minimised ones. Among those, the ones already minimised keep their
// order (`lastMin`: their addresses as last shown) and a window minimised just now goes last.
function minimisedLast(list, lastMin) {
    var prev = Array.isArray(lastMin) ? lastMin : [];
    var live = [], mins = [];
    for (var i = 0; i < list.length; i++)
        (list[i].minimised === true ? mins : live).push({ w: list[i], i: i });
    if (mins.length === 0)
        return list;
    mins.sort(function (a, b) {
        var pa = prev.indexOf(a.w.address), pb = prev.indexOf(b.w.address);
        if (pa < 0) pa = 1e9;
        if (pb < 0) pb = 1e9;
        return pa !== pb ? pa - pb : a.i - b.i;
    });
    return live.concat(mins).map(function (k) { return k.w; });
}

// "Screen order": the windows left to right as they sit on screen (top to bottom within a
// column), as the scrolling layout's columns run and any layout's tiles sit. A minimised window
// has left the layout, so it keeps its place from `lastOrder` (the addresses in the order last
// shown): right after the window that was before it. One never shown goes last.
function screenOrder(list, lastOrder) {
    var prev = Array.isArray(lastOrder) ? lastOrder : [];
    var live = [], mins = [];
    for (var i = 0; i < list.length; i++)
        (list[i].minimised === true ? mins : live).push({ w: list[i], i: i });
    live.sort(function (a, b) {
        var ax = typeof a.w.x === "number" ? a.w.x : 0, bx = typeof b.w.x === "number" ? b.w.x : 0;
        if (ax !== bx)
            return ax - bx;
        var ay = typeof a.w.y === "number" ? a.w.y : 0, by = typeof b.w.y === "number" ? b.w.y : 0;
        return ay !== by ? ay - by : a.i - b.i;
    });
    var out = live.map(function (k) { return k.w; });
    mins.sort(function (a, b) {
        var pa = prev.indexOf(a.w.address), pb = prev.indexOf(b.w.address);
        if (pa < 0) pa = 1e9;
        if (pb < 0) pb = 1e9;
        return pa !== pb ? pa - pb : a.i - b.i;
    });
    for (var j = 0; j < mins.length; j++) {
        var m = mins[j].w, at = prev.indexOf(m.address);
        if (at < 0) {
            out.push(m);
            continue;
        }
        var put = 0;
        for (var p = at - 1; p >= 0; p--) {
            var k = indexOfAddr(out, prev[p]);
            if (k >= 0) {
                put = k + 1;
                break;
            }
        }
        out.splice(put, 0, m);
    }
    return out;
}

function indexOfAddr(list, addr) {
    for (var i = 0; i < list.length; i++)
        if (list[i].address === addr)
            return i;
    return -1;
}

// A notification's app matches a window's app: the notification's app name or icon against the
// window's class, desktop id or app name, ignoring case. note: {appName, appIcon}; app: {cls,
// id, name}.
function notifMatches(note, app) {
    if (!note || !app)
        return false;
    var keys = [note.appName, note.appIcon].filter(function (s) { return typeof s === "string" && s !== ""; })
        .map(function (s) { return s.toLowerCase().replace(/\.desktop$/, ""); });
    var mine = [app.cls, app.id, app.name].filter(function (s) { return typeof s === "string" && s !== ""; })
        .map(function (s) { return s.toLowerCase().replace(/\.desktop$/, ""); });
    for (var i = 0; i < keys.length; i++)
        for (var j = 0; j < mine.length; j++)
            if (keys[i] === mine[j])
                return true;
    return false;
}

// --- windows that want attention (a dot on their tab's icon) ---
// A notification backend's `latest` ({appName, appIcon, summary}) is re-sent with every state
// update, and falls back to an older one when the newest is dismissed: neither is a new
// notification. seen: {keys (the recent ones, newest last), count} from the last call, or null.
// Returns {fresh (a new notification arrived), seen (to pass next time)}.
var SEEN_KEYS = 20;
function noteArrived(seen, latest, count) {
    var s = seen && Array.isArray(seen.keys) ? seen : { keys: [], count: 0 };
    var c = typeof count === "number" ? count : 0;
    if (!latest)
        return { fresh: false, seen: { keys: s.keys, count: c } };
    // Without a summary (hidden while the screen is shared or captured, or never sent) the text
    // can't tell one notification from another: only a rise in the count says it is new. It
    // isn't remembered, so the full one coming back isn't new either.
    if (typeof latest.summary !== "string" || latest.summary === "")
        return { fresh: c > s.count, seen: { keys: s.keys, count: c } };
    var key = [latest.appName, latest.appIcon, latest.summary].map(function (x) { return typeof x === "string" ? x : ""; }).join("\u0001");
    var last = s.keys.length > 0 ? s.keys[s.keys.length - 1] : null;
    // The same as last time is new only when the count rose (the same text again); one seen a
    // little earlier is the newest dismissed.
    var fresh = key === last ? c > s.count : s.keys.indexOf(key) < 0;
    var keys = s.keys.filter(function (k) { return k !== key; });
    keys.push(key);
    return { fresh: fresh, seen: { keys: keys.slice(-SEEN_KEYS), count: c } };
}

// "Last activity": the recent notifications ({appName, appIcon, at}), newest last, with one
// more from `latest` at `now`; at most NOTES_MAX kept.
var NOTES_MAX = 50;
function withNote(notes, latest, now) {
    var n = Array.isArray(notes) ? notes.slice(Math.max(0, notes.length - (NOTES_MAX - 1))) : [];
    n.push({ appName: String(latest && latest.appName || ""), appIcon: String(latest && latest.appIcon || ""), at: now });
    return n;
}

// The alerts (address → true) with `addrs` added, leaving out the focused window (it is already
// being looked at); the same object when nothing changes.
function withAlerts(alerts, addrs, focused) {
    var out = null, f = normAddr(focused);
    for (var i = 0; i < (addrs || []).length; i++) {
        var a = normAddr(addrs[i]);
        if (a === null || a === f || (out || alerts)[a] === true)
            continue;
        out = out || Object.assign({}, alerts);
        out[a] = true;
    }
    return out || alerts;
}

// The alerts kept: those of windows still open (`live`, addresses) that aren't focused; the same
// object when nothing changes.
function keepAlerts(alerts, live, focused) {
    var f = normAddr(focused), open = {}, out = {}, dropped = false;
    for (var i = 0; i < (live || []).length; i++)
        open[normAddr(live[i])] = true;
    for (var a in alerts) {
        if (a !== f && open[a] === true)
            out[a] = true;
        else
            dropped = true;
    }
    return dropped ? out : alerts;
}
