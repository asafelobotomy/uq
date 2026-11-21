.pragma library

// Resize grips in the gaps between tiled windows (shell/windows/ResizeGrips.qml). Pure, so
// tests/qml/tst_grips.qml covers it without Hyprland.
//
// Windows are read as tiles: windows sharing a left edge and width form a column (the scrolling
// layout's columns; a dwindle or master split stacked the same way counts too), top to bottom.
// A drag moves only the gap: the two windows on either side of it share what they had, so their
// far edges stay put.
//   - Between two columns side by side, a window gap apart and both on the screen: a "col" grip,
//     the height they share.
//     Dragging it widens one column by what the other loses (both are resized).
//   - Between two windows stacked in a column, a window gap apart: a "row" grip, the column's
//     width. Dragging it sets the upper window's height; Hyprland gives the lower one the rest.
// Each grip covers the whole gap between the two frames (borders included in the frames), so
// the pointer finds it anywhere in the gap.

// How far off a gap may be and still count (rounding, odd sizes).
var TOLERANCE = 3;
// Narrowest a window may be dragged.
var MIN_W = 120;
var MIN_H = 80;

// The columns of `wins` ([{address, x, y, w, h, fullscreen}], global): [{x, w, wins}], left to
// right, each column's windows top to bottom.
function columns(wins) {
    var cols = [];
    var list = (wins || []).slice().sort(function (a, b) { return a.x - b.x || a.y - b.y; });
    for (var i = 0; i < list.length; i++) {
        var w = list[i], c = null;
        for (var j = 0; j < cols.length && c === null; j++)
            if (Math.abs(cols[j].x - w.x) <= TOLERANCE && Math.abs(cols[j].w - w.w) <= TOLERANCE)
                c = cols[j];
        if (c === null) {
            c = { x: w.x, w: w.w, wins: [] };
            cols.push(c);
        }
        c.wins.push(w);
    }
    for (var k = 0; k < cols.length; k++)
        cols[k].wins.sort(function (a, b) { return a.y - b.y; });
    return cols;
}

function _top(c) { return c.wins[0].y; }
function _bottom(c) { var l = c.wins[c.wins.length - 1]; return l.y + l.h; }

// The grips between `wins` on a monitor `mon` ({x, y, width, height}, global): [{kind, x, y, w,
// h (in the monitor's own coordinates), address (the window before the gap: left or above),
// from: {w, h} (its size now), fullscreen (its state now), other: {address, w, h, fullscreen}
// (the window after the gap), min, max (the range a drag may give the first window's width for
// "col", height for "row", leaving the other at least its minimum)}]. `border` is the frame's
// border width and `gap` the space between two frames.
function grips(wins, mon, border, gap) {
    var out = [];
    if (!mon)
        return out;
    var b = border || 0, g = gap || 0;
    var cols = columns(wins);
    function add(kind, x, y, w, h, target, next, min, max) {
        // Clipped to the monitor; nothing when none of it is on screen.
        var x0 = Math.max(x, mon.x), y0 = Math.max(y, mon.y);
        var x1 = Math.min(x + w, mon.x + mon.width), y1 = Math.min(y + h, mon.y + mon.height);
        if (x1 - x0 < 1 || y1 - y0 < 1 || max < min)
            return;
        out.push({ kind: kind, x: x0 - mon.x, y: y0 - mon.y, w: x1 - x0, h: y1 - y0,
                   address: target.address, from: { x: target.x, w: target.w, h: target.h },
                   fullscreen: target.fullscreen || 0,
                   other: { address: next.address, w: next.w, h: next.h, fullscreen: next.fullscreen || 0 },
                   min: min, max: max });
    }
    for (var i = 0; i + 1 < cols.length; i++) {
        var l = cols[i], r = cols[i + 1];
        var left = l.x + l.w + b, right = r.x - b;
        if (Math.abs(right - left - g) > TOLERANCE)
            continue;
        // Both columns on this screen: a column off its edge (on another screen page) can't be
        // shared with, and the layout keeps the focused one in view, so a drag there would
        // move the far edges instead of the gap.
        if (l.x - b < mon.x - TOLERANCE || r.x + r.w + b > mon.x + mon.width + TOLERANCE)
            continue;
        var top = Math.max(_top(l), _top(r)) - b, bottom = Math.min(_bottom(l), _bottom(r)) + b;
        if (bottom - top < 1)
            continue;
        add("col", left, top, right - left, bottom - top, l.wins[0], r.wins[0], MIN_W, l.w + r.w - MIN_W);
    }
    for (var c = 0; c < cols.length; c++) {
        var ws = cols[c].wins;
        for (var k = 0; k + 1 < ws.length; k++) {
            var t = ws[k], u = ws[k + 1];
            var upper = t.y + t.h + b, lower = u.y - b;
            if (Math.abs(lower - upper - g) > TOLERANCE)
                continue;
            add("row", cols[c].x - b, upper, cols[c].w + 2 * b, lower - upper, t, u,
                MIN_H, t.h + u.h - MIN_H);
        }
    }
    return out;
}

// The size to give grip `gr`'s first window for a drag of (dx, dy) from where it started: {w, h}.
function dragSize(gr, dx, dy) {
    function clamp(v) { return Math.round(Math.max(gr.min, Math.min(gr.max, v))); }
    if (gr.kind === "col")
        return { w: clamp(gr.from.w + dx), h: gr.from.h };
    return { w: gr.from.w, h: clamp(gr.from.h + dy) };
}

// What the bridge's resize takes for a stacked drag: [{address, w, h}], the upper window only
// (Hyprland gives the lower one the rest).
function resizes(gr, dx, dy) {
    var first = dragSize(gr, dx, dy);
    return [{ address: gr.address, w: first.w, h: first.h }];
}

// What the bridge's columns action takes for a column drag: {windows: [{address, fraction}],
// pin: {address, x}}. Each column's width is a fraction of the room columns share (`room`: the
// screen's width less gaps_out on each side); a column takes its window's width plus `extra`
// (its borders and gaps_in on each side). The two fractions add up to what the columns had, the
// one that grows goes first (so the pair never comes up short, which the layout would re-centre),
// and the left window's left edge is pinned where it was: only the gap moves.
function columnsCall(gr, dx, room, extra) {
    var w = dragSize(gr, dx, 0).w;
    var both = gr.from.w + gr.other.w + 2 * extra;
    function f(px) { return Math.round(px / room * 10000) / 10000; }
    var fa = f(w + extra), fb = Math.round((both / room - fa) * 10000) / 10000;
    var a = { address: gr.address, fraction: fa }, b = { address: gr.other.address, fraction: fb };
    return { windows: w >= gr.from.w ? [a, b] : [b, a], pin: { address: gr.address, x: gr.from.x } };
}

