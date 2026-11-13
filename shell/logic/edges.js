.pragma library

// Screen-edge indicators (shell/edge/): what lies to the left and right of what a monitor shows,
// where on a screen's side edge an indicator may sit, and how wheel turns become steps. Pure, so
// tests/qml/tst_edges.qml covers it without Hyprland.
//
// The rules:
//   - The reference is the most recently focused window on the monitor's active workspace
//     (lowest focusHistoryID), or the window just moved to (`refAddr`), so quick wheel steps
//     don't wait for Hyprland's next reply. A floating or pinned reference is replaced by the
//     most recent tiled window, since floating windows sit outside the layout.
//   - The neighbour on a side is a tiled, visible window on that workspace whose centre lies on
//     that side of the reference's centre. This one rule covers every layout: the scrolling
//     layout reports columns out of view at their real, off-screen positions, and dwindle and
//     master tiles sit side by side. Windows that overlap the reference vertically come first,
//     then the nearest centre, then the nearest vertically, then the most recently focused.
//   - With no such window, the neighbour is the nearest workspace on the same monitor in that
//     direction (by id, normal workspaces only) that has a window; going there focuses its most
//     recently focused window.

// Pixels a centre must be past the reference's centre to count as being on that side.
var SIDE_TOLERANCE = 2;
// One wheel notch (QWheelEvent angleDelta).
var NOTCH = 120;

// A window as these rules read it, from a toplevel's lastIpcObject plus its title. Returns null
// without an address or geometry.
function fromIpc(o, title) {
    // `at` and `size` arrive from Quickshell as list-likes, not JS arrays.
    if (!o || typeof o !== "object" || !_pair(o.at) || !_pair(o.size))
        return null;
    var addr = typeof o.address === "string" ? o.address : "";
    if (addr === "")
        return null;
    var ws = o.workspace && typeof o.workspace.id === "number" ? o.workspace.id : 0;
    return {
        address: addr,
        cls: typeof o["class"] === "string" ? o["class"] : "",
        title: typeof title === "string" && title !== "" ? title
            : (typeof o.title === "string" ? o.title : ""),
        ws: ws,
        x: Number(o.at[0]) || 0,
        y: Number(o.at[1]) || 0,
        w: Number(o.size[0]) || 0,
        h: Number(o.size[1]) || 0,
        floating: o.floating === true,
        pinned: o.pinned === true,
        hidden: o.hidden === true,
        fullscreen: typeof o.fullscreen === "number" ? o.fullscreen : 0,
        history: typeof o.focusHistoryID === "number" ? o.focusHistoryID : 1e9
    };
}

function _pair(v) {
    return !!v && typeof v === "object" && v.length >= 2;
}

function _cx(w) { return w.x + w.w / 2; }
function _cy(w) { return w.y + w.h / 2; }
function _tiled(w) { return !w.floating && !w.pinned; }

function _mostRecent(list) {
    var best = null;
    for (var i = 0; i < list.length; i++)
        if (best === null || list[i].history < best.history)
            best = list[i];
    return best;
}

function _find(wins, addr) {
    for (var i = 0; i < wins.length; i++)
        if (wins[i].address === addr)
            return wins[i];
    return null;
}

// The reference window for workspace `ws` (see the rules above), or null when it has no tiled
// window.
function reference(wins, ws, refAddr) {
    var on = [];
    for (var i = 0; i < wins.length; i++)
        if (wins[i].ws === ws && !wins[i].hidden)
            on.push(wins[i]);
    var r = refAddr ? _find(on, refAddr) : null;
    if (r === null)
        r = _mostRecent(on);
    if (r !== null && _tiled(r))
        return r;
    return _mostRecent(on.filter(_tiled));
}

// The tiled window on `side` ("left" | "right") of the reference on workspace `ws`, or null.
function windowBeside(wins, ws, side, refAddr) {
    var ref = reference(wins, ws, refAddr);
    if (ref === null)
        return null;
    var rc = _cx(ref), ry = _cy(ref);
    var best = null, bestKey = null;
    for (var i = 0; i < wins.length; i++) {
        var w = wins[i];
        if (w === ref || w.ws !== ws || w.hidden || !_tiled(w))
            continue;
        var dx = side === "left" ? rc - _cx(w) : _cx(w) - rc;
        if (dx <= SIDE_TOLERANCE)
            continue;
        var overlaps = Math.min(w.y + w.h, ref.y + ref.h) - Math.max(w.y, ref.y) > 0;
        var key = [overlaps ? 0 : 1, dx, Math.abs(_cy(w) - ry), w.history];
        if (best === null || _less(key, bestKey)) {
            best = w;
            bestKey = key;
        }
    }
    return best;
}

function _less(a, b) {
    for (var i = 0; i < a.length; i++)
        if (a[i] !== b[i])
            return a[i] < b[i];
    return false;
}

// The nearest workspace id in `monitorWs` (the ids on this monitor) on `side` of `ws` that has a
// visible window, with that workspace's most recently focused window; or null.
function workspaceBeside(wins, ws, side, monitorWs) {
    var bestId = null;
    for (var i = 0; i < monitorWs.length; i++) {
        var id = monitorWs[i];
        if (typeof id !== "number" || id < 1 || id === ws)
            continue;
        if (side === "left" ? id > ws : id < ws)
            continue;
        if (bestId !== null && (side === "left" ? id < bestId : id > bestId))
            continue;
        var has = false;
        for (var j = 0; j < wins.length && !has; j++)
            has = wins[j].ws === id && !wins[j].hidden;
        if (has)
            bestId = id;
    }
    if (bestId === null)
        return null;
    var on = [];
    for (var k = 0; k < wins.length; k++)
        if (wins[k].ws === bestId && !wins[k].hidden)
            on.push(wins[k]);
    return { ws: bestId, win: _mostRecent(on) };
}

// What the indicator on `side` shows and goes to: {address, cls, title, workspace} where
// `workspace` is the id when it is another workspace's window and 0 when it is a window in the
// same layout. Null when there is nothing on that side. `ws` is the monitor's active workspace;
// with `refAddr` the reference's own workspace is used instead.
function neighbour(wins, ws, side, monitorWs, refAddr) {
    if (side !== "left" && side !== "right" || !Array.isArray(wins))
        return null;
    if (refAddr) {
        var r = _find(wins, refAddr);
        if (r !== null && r.ws >= 1)
            ws = r.ws;
    }
    if (typeof ws !== "number" || ws < 1)
        return null;
    var w = windowBeside(wins, ws, side, refAddr);
    if (w !== null)
        return { address: w.address, cls: w.cls, title: w.title, workspace: 0 };
    var o = workspaceBeside(wins, ws, side, Array.isArray(monitorWs) ? monitorWs : []);
    if (o === null || o.win === null)
        return null;
    return { address: o.win.address, cls: o.win.cls, title: o.win.title, workspace: o.ws };
}

function _intersect(a, b) {
    var x = Math.max(a.x, b.x), y = Math.max(a.y, b.y);
    var r = Math.min(a.x + a.w, b.x + b.w), btm = Math.min(a.y + a.h, b.y + b.h);
    return r > x && btm > y ? { x: x, y: y, w: r - x, h: btm - y } : null;
}

function _contains(outer, inner) {
    return inner.x >= outer.x && inner.y >= outer.y && inner.x + inner.w <= outer.x + outer.w
        && inner.y + inner.h <= outer.y + outer.h;
}

// The "window peek" style: whether the neighbour `n` (from neighbour()) should show as a
// standing slice at the edge. Only while the reference window is fullscreen or maximized, and
// only when nothing of the neighbour can be seen on the monitor already: off-screen, behind the
// reference, or on another workspace. (A maximized column in the scrolling layout leaves its
// neighbour's frame in view, so no slice is added there.) `mon` is the monitor's rectangle
// {x, y, width, height} in the same (logical, global) coordinates as the windows.
function peekWanted(wins, ws, n, mon, refAddr) {
    if (!n || !mon || !Array.isArray(wins))
        return false;
    if (refAddr) {
        var r0 = _find(wins, refAddr);
        if (r0 !== null && r0.ws >= 1)
            ws = r0.ws;
    }
    var ref = reference(wins, ws, refAddr);
    if (ref === null || !(ref.fullscreen > 0))
        return false;
    if (n.workspace > 0)
        return true;
    var w = _find(wins, n.address);
    if (w === null)
        return false;
    var seen = _intersect(w, { x: mon.x, y: mon.y, w: mon.width, h: mon.height });
    return seen === null || _contains(ref, seen);
}

// The part of `me`'s `side` edge that no other screen touches, as {top, bottom} offsets from
// the screen's top and bottom (layer-shell margins), the longest free run when another screen
// covers the middle; null when another screen covers it all. Screens are {x, y, width, height}
// in logical pixels.
function freeSpan(screens, me, side) {
    if (!me || !(me.height > 0))
        return null;
    var edgeX = side === "left" ? me.x : me.x + me.width;
    var taken = [];
    for (var i = 0; i < (screens ? screens.length : 0); i++) {
        var s = screens[i];
        if (!s || s === me || (s.x === me.x && s.y === me.y && s.width === me.width && s.height === me.height))
            continue;
        var touchX = side === "left" ? s.x + s.width : s.x;
        if (Math.abs(touchX - edgeX) > 1)
            continue;
        var a = Math.max(s.y, me.y), b = Math.min(s.y + s.height, me.y + me.height);
        if (b > a)
            taken.push([a - me.y, b - me.y]);
    }
    taken.sort(function (p, q) { return p[0] - q[0]; });
    var best = null, at = 0;
    function consider(from, to) {
        if (to - from > 0 && (best === null || to - from > best[1] - best[0]))
            best = [from, to];
    }
    for (var j = 0; j < taken.length; j++) {
        consider(at, taken[j][0]);
        at = Math.max(at, taken[j][1]);
    }
    consider(at, me.height);
    if (best === null)
        return null;
    return { top: best[0], bottom: me.height - best[1] };
}

// Wheel turns into whole steps: `acc` is what is left over from earlier events, `delta` this
// event's angleDelta (x or y, whichever is larger in size). Returns {steps, acc}. Every turn,
// either way, steps towards the edge the pointer is on.
function wheelSteps(acc, delta) {
    var total = (Number(acc) || 0) + Math.abs(Number(delta) || 0);
    var steps = Math.floor(total / NOTCH);
    return { steps: steps, acc: total - steps * NOTCH };
}

// Where the tab's top goes: centred on the pointer, kept inside [0, span - tabH].
function tabTop(pointerY, tabH, span) {
    var t = pointerY - tabH / 2;
    return Math.max(0, Math.min(t, span - tabH));
}

// The tab's accessible name and tooltip: the title (or app class), and the workspace when the
// neighbour is on another one. While the screen is shared only the workspace or a generic name
// is given, so titles don't reach the recording.
function label(n, sharing, side) {
    if (!n)
        return "";
    var name = sharing ? "" : (n.title || n.cls || "");
    if (n.workspace > 0)
        return name !== "" ? name + " (workspace " + n.workspace + ")" : "Workspace " + n.workspace;
    return name !== "" ? name : (side === "left" ? "Window on the left" : "Window on the right");
}
