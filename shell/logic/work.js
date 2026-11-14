.pragma library

// Workspace id to pass as `from_ws` for Minimise: only real (positive, integer) workspaces;
// windows on special workspaces cannot be minimised again.
function minimiseFrom(ws) {
    if (!ws || typeof ws.id !== "number" || ws.id !== Math.floor(ws.id) || ws.id <= 0)
        return null;
    return ws.id;
}

// "Move to workspace" targets, as integers.
function wsChoices() {
    var out = [];
    for (var i = 1; i <= 10; i++)
        out.push(i);
    return out;
}

// Normalise a Hyprland address to 0x… and validate it against the bridge's pattern.
function normAddr(a) {
    var s = String(a);
    if (s.indexOf("0x") !== 0)
        s = "0x" + s;
    return /^0x[0-9a-f]{1,16}$/.test(s) ? s : null;
}

// Build the [name, args] pair for Bridge.call("Dispatch", ...) from a captured target
// ({addr, ws}); null when the address is invalid or the action is not allowed.
function buildDispatch(name, target, extra) {
    if (!target)
        return null;
    var addr = normAddr(target.addr);
    if (addr === null)
        return null;
    var args = { address: addr };
    var ex = extra || ({});
    switch (name) {
    case "close":
    case "toggle_float":
    case "pin":
        break;
    case "fullscreen":
        if (ex.mode !== "full" && ex.mode !== "maximized")
            return null;
        args.mode = ex.mode;
        break;
    case "move":
        if (typeof ex.workspace !== "number" || ex.workspace !== Math.floor(ex.workspace)
                || ex.workspace < 1 || ex.workspace > 10)
            return null;
        args.workspace = ex.workspace;
        break;
    case "minimise":
        var from = minimiseFrom({ id: target.ws });
        if (from === null)
            return null;
        args.from_ws = from;
        break;
    default:
        return null;
    }
    return [name, args];
}

// Up to this many open windows show as their own button in the Work island; past it the island
// folds to one icon button until it is clicked open.
var MAX_TABS = 5;

// The Work island's window buttons. windows: [{address, cls, title, workspace (name), minimised,
// monitor}] as the bar reads them from Hyprland (untrusted titles, unchanged here); monitor: this
// bar's monitor name ("" for every monitor); focused: the focused window's address; expanded:
// the folded island was clicked open. Windows on other special workspaces (a scratchpad) are left
// out; minimised ones stay, marked. Returns { tabs: [{address, cls, title, focused, minimised}],
// folded: bool, count } with tabs in Hyprland's order.
function taskView(windows, monitor, focused, expanded) {
    var tabs = [];
    var list = windows || [];
    for (var i = 0; i < list.length; i++) {
        var w = list[i];
        if (!w)
            continue;
        var addr = normAddr(w.address);
        if (addr === null)
            continue;
        if (monitor && w.monitor && w.monitor !== monitor)
            continue;
        var ws = typeof w.workspace === "string" ? w.workspace : "";
        if (ws.indexOf("special:") === 0 && w.minimised !== true)
            continue;
        tabs.push({
            address: addr,
            cls: typeof w.cls === "string" ? w.cls : "",
            title: typeof w.title === "string" ? w.title : "",
            focused: addr === normAddr(focused),
            minimised: w.minimised === true
        });
    }
    return { tabs: tabs, folded: tabs.length > MAX_TABS && expanded !== true, count: tabs.length };
}

// Width each button's title may take: the island's share of the bar split between the buttons,
// never less than a short word.
function titleWidth(barWidth, count) {
    var share = (barWidth > 0 ? barWidth : 1280) * 0.4;
    return Math.max(48, Math.floor(share / Math.max(1, count)));
}

// --- The Open windows island: one pill per window, like browser tabs ---

// The windows the strip shows: on monitor `monitor` ("" for any), on its active workspace
// `wsId`, and the windows minimised from that workspace (shown dimmed). windows: [{address, cls,
// title, wsId, minimised, minimisedFrom, monitor, pid}] as the bar reads them from Hyprland.
// Optional geometry is passed through for "Screen order" and the Full/Half buttons: x, y, w, h
// (Hyprland's `at` and `size`), floating, fullscreen (Hyprland's state number), full (the window
// spans its screen's width), scrolling (its workspace uses the scrolling layout).
// Returns [{address, cls, title, minimised, focused, pid, x, y, w, h, floating, fullscreen, full,
// scrolling}] in Hyprland's order.
function windowsHere(windows, monitor, wsId, focused) {
    var out = [];
    var list = windows || [];
    var f = normAddr(focused);
    for (var i = 0; i < list.length; i++) {
        var w = list[i];
        if (!w)
            continue;
        var addr = normAddr(w.address);
        if (addr === null)
            continue;
        if (monitor && w.monitor && w.monitor !== monitor && w.minimised !== true)
            continue;
        var here = w.minimised === true ? w.minimisedFrom === wsId : w.wsId === wsId;
        if (!here || typeof wsId !== "number")
            continue;
        out.push({
            address: addr,
            cls: typeof w.cls === "string" ? w.cls : "",
            title: typeof w.title === "string" ? w.title : "",
            minimised: w.minimised === true,
            focused: addr === f,
            pid: typeof w.pid === "number" && w.pid > 0 ? w.pid : 0,
            x: typeof w.x === "number" ? w.x : 0,
            y: typeof w.y === "number" ? w.y : 0,
            w: typeof w.w === "number" ? w.w : 0,
            h: typeof w.h === "number" ? w.h : 0,
            floating: w.floating === true,
            fullscreen: typeof w.fullscreen === "number" ? w.fullscreen : 0,
            full: w.full === true,
            scrolling: w.scrolling === true
        });
    }
    return out;
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
    return mode === "screen" ? screenOrder(out, c.lastOrder) : out;
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

// How `n` pills fit a strip `width` wide. m: {minPill (the narrowest tab: Icon), maxPill, gap,
// arrow (a scroll arrow), chip (the "+N" button), medium (optional: a Medium tab's width)}. With
// `focus` (the focused pill's index, -1 for none) and m.medium, the focused pill grows to Medium
// (or as near as the strip allows) and the others share what is left. Returns {mode, pillW,
// focusW, shown}: "empty" (no windows); "pills" (all fit: each pillW wide, at most maxPill, the
// focused one focusW); "scroll" (only `shown` fit at a time, between two arrows, with "+N"
// listing the rest); "compact" (not even one pill fits: the lead window's icon, which lists them
// all).
function strip(n, width, m, focus) {
    if (n <= 0)
        return { mode: "empty", pillW: 0, focusW: 0, shown: 0 };
    var hasF = typeof focus === "number" && focus >= 0 && focus < n
        && typeof m.medium === "number" && m.medium > m.minPill;
    if (n * m.minPill + (n - 1) * m.gap <= width) {
        var even = Math.min(m.maxPill, Math.floor((width - (n - 1) * m.gap) / n));
        if (!hasF || n === 1 || even >= m.medium)
            return { mode: "pills", shown: n, pillW: even, focusW: even };
        var fw = Math.min(m.medium, width - (n - 1) * (m.minPill + m.gap));
        var rest = Math.min(m.maxPill, Math.floor((width - fw - (n - 1) * m.gap) / (n - 1)));
        return { mode: "pills", shown: n, pillW: rest, focusW: fw };
    }
    var avail = width - 2 * (m.arrow + m.gap) - (m.chip + m.gap);
    if (hasF) {
        var f = Math.min(m.medium, avail);
        if (f < m.minPill)
            return { mode: "compact", pillW: 0, focusW: 0, shown: 0 };
        var others = Math.min(n - 1, Math.floor((avail - f) / (m.minPill + m.gap)));
        var pw = others > 0 ? Math.min(m.maxPill, Math.floor((avail - f - others * m.gap) / others)) : 0;
        return { mode: "scroll", shown: others + 1, pillW: pw, focusW: f };
    }
    var k = Math.floor((avail + m.gap) / (m.minPill + m.gap));
    if (k < 1)
        return { mode: "compact", pillW: 0, focusW: 0, shown: 0 };
    k = Math.min(k, n);
    var w = Math.floor((avail - (k - 1) * m.gap) / k);
    return { mode: "scroll", shown: k, pillW: w, focusW: w };
}

// Whether window `o` (a toplevel's lastIpcObject) spans most of the usable width of its monitor
// `mon` (that monitor's lastIpcObject: physical width, scale and reserved [left, top, right,
// bottom]): a Full column in the scrolling layout.
function spansScreen(o, mon) {
    if (!o || !o.size || !mon || !(mon.width > 0))
        return false;
    var r = mon.reserved && mon.reserved.length >= 4 ? mon.reserved : [0, 0, 0, 0];
    var usable = mon.width / (mon.scale > 0 ? mon.scale : 1) - (Number(r[0]) || 0) - (Number(r[2]) || 0);
    return Number(o.size[0]) >= usable * 0.8;
}

// Which controls a tab `w` wide has room for. m: {small, medium, large} (the narrowest width of
// each). "icon": the app icon, which turns into a close button under the pointer; "small": icon,
// Full or Half (whichever the window isn't, under the pointer) and close; "medium": icon, Half,
// Full and close; "large": Medium plus the title.
function tabTier(w, m) {
    if (w >= m.large)
        return "large";
    if (w >= m.medium)
        return "medium";
    if (w >= m.small)
        return "small";
    return "icon";
}

// The first pill shown while scrolling to show the focused pill `keep` when it was out of view:
// centred, so the windows on both sides of it show as they sit on screen. Kept in range.
function centreOffset(n, shown, keep) {
    var max = Math.max(0, n - shown);
    if (keep < 0 || keep >= n)
        return 0;
    return Math.max(0, Math.min(keep - Math.floor((shown - 1) / 2), max));
}

// The first pill shown while scrolling, kept in range and moved just enough to show pill
// `keep` (the focused one; -1 for none).
function scrollOffset(offset, n, shown, keep) {
    var max = Math.max(0, n - shown);
    var o = Math.max(0, Math.min(offset, max));
    if (keep >= 0 && keep < n) {
        if (keep < o)
            o = keep;
        else if (keep >= o + shown)
            o = keep - shown + 1;
    }
    return o;
}
