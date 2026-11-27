.pragma library
.import "worksort.js" as Sort

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

// From worksort.js.
var normAddr = Sort.normAddr;
var SORTS = Sort.SORTS;
var DEFAULT_SORT = Sort.DEFAULT_SORT;
var SORT_NAMES = Sort.SORT_NAMES;
var ORDER_NAMES = Sort.ORDER_NAMES;
var sortHasOrder = Sort.sortHasOrder;
var CLOSES = Sort.CLOSES;
var CLOSE_NAMES = Sort.CLOSE_NAMES;
var closeAction = Sort.closeAction;
var sortWindows = Sort.sortWindows;
var screenOrder = Sort.screenOrder;
var notifMatches = Sort.notifMatches;
var noteArrived = Sort.noteArrived;
var NOTES_MAX = Sort.NOTES_MAX;
var withNote = Sort.withNote;
var withAlerts = Sort.withAlerts;
var keepAlerts = Sort.keepAlerts;

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

// Windows an app opens from one of its windows (preferences, a confirmation, a file chooser)
// share that window's tab. Hyprland floats such a window (it has a parent) but doesn't say which
// window is the parent, so a floating window counts as a child of another window of the same
// process (pid) opened before it: the one that had focus when it opened (`from`, address ->
// address), else the last one opened before it. Both are minimised or neither. `list` is
// windowsHere's; `opened` maps address -> opening order. Returns the list without the children,
// each window with `children` (addresses, oldest first; [] for none) and `focused` when it or one
// of its children is.
function groupChildren(list, opened, from) {
    var src = list || [], ord = opened || {}, fr = from || {};
    function seq(w) { return typeof ord[w.address] === "number" ? ord[w.address] : Number.MAX_SAFE_INTEGER; }
    var parentOf = {};
    for (var i = 0; i < src.length; i++) {
        var c = src[i];
        if (!c.floating || !(c.pid > 0))
            continue;
        var best = null;
        for (var j = 0; j < src.length; j++) {
            var p = src[j];
            if (p === c || p.pid !== c.pid || p.minimised !== c.minimised || seq(p) >= seq(c))
                continue;
            if (fr[c.address] === p.address) {
                best = p;
                break;
            }
            if (best === null || seq(p) > seq(best))
                best = p;
        }
        if (best !== null)
            parentOf[c.address] = best.address;
    }
    // A child's child belongs to the same top window.
    function top(a) {
        var seen = 0;
        while (parentOf[a] !== undefined && seen++ < 32)
            a = parentOf[a];
        return a;
    }
    var kids = {}, out = [];
    for (var k = 0; k < src.length; k++)
        if (parentOf[src[k].address] !== undefined) {
            var t = top(src[k].address);
            (kids[t] = kids[t] || []).push(src[k]);
        }
    for (var m = 0; m < src.length; m++) {
        var w = src[m];
        if (parentOf[w.address] !== undefined)
            continue;
        var mine = (kids[w.address] || []).slice().sort(function (a, b) { return seq(a) - seq(b); });
        var g = Object.assign({}, w);
        g.children = mine.map(function (x) { return x.address; });
        g.focused = w.focused || mine.some(function (x) { return x.focused; });
        out.push(g);
    }
    return out;
}

// How `n` pills fit a strip `width` wide. m: {minPill (the narrowest tab: Icon), maxPill, gap,
// arrow (a scroll arrow), chip (the "+N" button), medium (optional: a Medium tab's width)}. With
// `focus` (the focused pill's index, -1 for none) and m.medium, the focused pill grows to Medium
// (or as near as the strip allows) and the others share what is left. Returns {mode, pillW,
// focusW, shown}: "empty" (no windows); "pills" (all fit: each pillW wide, at most maxPill, the
// focused one focusW); "scroll" (only `shown` fit at a time, between two arrows, with "+N"
// listing the rest); "compact" (not even one pill fits: the lead window's icon, which lists them
// all). `fixed` (optional) is how many of the `n` are icon-sized always (minimised windows, last in
// the queue): when everything fits they take just minPill each and the others share the rest.
function strip(n, width, m, focus, fixed) {
    if (n <= 0)
        return { mode: "empty", pillW: 0, focusW: 0, shown: 0 };
    var fx = typeof fixed === "number" && fixed > 0 && fixed < n ? Math.floor(fixed) : 0;
    if (fx > 0 && n * m.minPill + (n - 1) * m.gap <= width) {
        var inner = strip(n - fx, width - fx * (m.minPill + m.gap), m, focus);
        return { mode: "pills", shown: n, pillW: inner.pillW, focusW: inner.focusW };
    }
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

// --- Dragging tabs into a new order ---

// A tab that can be dragged down onto the screen to join the page shown there: a tiled window in
// the scrolling layout (any order).
function canDropOnScreen(w) {
    return !!w && w.scrolling === true && w.floating !== true && w.minimised !== true;
}

// Whether the tab for window `w` can be dragged along the island: a tiled window in the
// scrolling layout, whose column is what moves. In any sort: a drop puts the island in screen
// order, the order the drag set.
function canDrag(w) {
    return !!w && w.scrolling === true && w.floating !== true && w.minimised !== true;
}

// A drag in the island's tabs (`shown`, from index `from` dropped at `to`) as a move in screen
// order (`screen`, the same windows left to right): {from, to} indices in `screen`. Dropped first
// or last, it goes first or last on screen; otherwise just after the tab on its left. In screen
// order it is the drag itself. null when nothing moves.
function screenMove(shown, screen, from, to) {
    if (!shown || !screen || from < 0 || from >= shown.length || to < 0 || to >= shown.length
            || from === to)
        return null;
    var me = shown[from];
    var at = function (w) {
        for (var i = 0; i < screen.length; i++)
            if (screen[i].address === w.address)
                return i;
        return -1;
    };
    var moved = shown.slice();
    moved.splice(from, 1);
    moved.splice(to, 0, me);
    var sFrom = at(me);
    if (sFrom < 0)
        return null;
    var sTo;
    if (to === 0) {
        sTo = 0;
    } else if (to === shown.length - 1) {
        sTo = screen.length - 1;
    } else {
        var l = at(moved[to - 1]);
        if (l < 0)
            return null;
        sTo = l < sFrom ? l + 1 : l;
    }
    return sTo === sFrom ? null : { from: sFrom, to: sTo };
}

// How many columns the window at `from` in `list` (the island's windows in screen order) moves
// when its tab is dropped at `to` (its index in the list once moved): the other columns before
// it after the move, less those before it now. Windows stacked in one column (same x) are one
// column; minimised and floating windows are no column. Negative: left.
function columnSteps(list, from, to) {
    if (!list || from < 0 || from >= list.length || to < 0 || to >= list.length || from === to)
        return 0;
    var me = list[from];
    function colsBefore(order, upto) {
        var xs = [];
        for (var i = 0; i < upto; i++) {
            var w = order[i];
            if (w.minimised === true || w.floating === true || Math.abs(w.x - me.x) <= 2)
                continue;
            var seen = false;
            for (var j = 0; j < xs.length && !seen; j++)
                seen = Math.abs(xs[j] - w.x) <= 2;
            if (!seen)
                xs.push(w.x);
        }
        return xs.length;
    }
    var moved = list.slice();
    moved.splice(from, 1);
    moved.splice(to, 0, me);
    return colsBefore(moved, to) - colsBefore(list, from);
}

// The index a tab dragged from `from` lands at when its centre is at `x` along a strip of tabs
// whose left edges are `lefts` and widths `widths` (as drawn before the drag): the slot whose
// tab centre it passed last.
function dropIndex(lefts, widths, from, x) {
    var to = from;
    for (var i = 0; i < lefts.length; i++) {
        var c = lefts[i] + widths[i] / 2;
        if (i < from && x < c)
            return i;
        if (i > from && x > c)
            to = i;
    }
    return to;
}

// The window a restored one lands after, to join the end of the queue: the rightmost column's
// window on workspace `ws` (ties: lowest, i.e. the last in a column), among windows that are
// tiled and not minimised, other than `self`. null when there is none. windows: as the bar reads
// them (address, wsId, minimised, floating, x, y).
function lastLive(windows, ws, self) {
    var best = null, me = normAddr(self);
    for (var i = 0; i < (windows || []).length; i++) {
        var w = windows[i], a = normAddr(w && w.address);
        if (a === null || a === me || w.minimised === true || w.floating === true || w.wsId !== ws)
            continue;
        var x = typeof w.x === "number" ? w.x : 0, y = typeof w.y === "number" ? w.y : 0;
        if (best === null || x > best.x || (x === best.x && y > best.y))
            best = { a: a, x: x, y: y };
    }
    return best === null ? null : best.a;
}
