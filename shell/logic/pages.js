.pragma library

.import "grips.js" as Grips

// Screen pages in the scrolling layout (shell/windows/ScreenPages.qml): the columns of a
// workspace, left to right, fall into pages a screen wide, and the view always shows one whole
// page, so no window is cut by the screen's edge. Pure, so tests/qml/tst_pages.qml covers it
// without Hyprland.
//
// A page starts at a column and takes the columns after it while they still fit in the screen's
// width: a Full column is a page of its own, two Half columns share one, and so do two columns
// a resize grip gave 60% and 40%. (Hyprland's own scrolling only makes sure the focused column
// is on screen, which can leave the one before it half on screen.)

// How far off a fit or a position may be and still count (rounding, odd sizes).
var TOLERANCE = 3;

// The pages of `wins` ([{address, x, y, w, h}], one workspace's tiled windows, global), given
// the width the columns share (`room`: the screen less its outer gaps) and what each column
// takes besides its window's width (`extra`: window gap and borders). `starts` ({address: true},
// optional): windows that began a page before, which still do, so a page doesn't change when one
// of its windows changes width (a Full window made Half stays where it was, alone on its screen,
// rather than joining a Half page before it); startsOf() gives them. Returns [{x (the first
// column's window x), cols: [{x, w, wins}], used (width taken)}], left to right.
function pages(wins, room, extra, starts) {
    var cols = Grips.columns(wins), out = [], cur = null;
    for (var i = 0; i < cols.length; i++) {
        var slot = cols[i].w + (extra || 0);
        var starter = !!starts && cols[i].wins.some(function (w) { return starts[w.address] === true; });
        if (cur === null || starter || cur.used + slot > room + TOLERANCE) {
            cur = { x: cols[i].x, cols: [], used: 0 };
            out.push(cur);
        }
        cur.cols.push(cols[i]);
        cur.used += slot;
    }
    return out;
}

// The windows beginning each page of `list` (pages()): {address: true}, to pass back as `starts`.
function startsOf(list) {
    var out = {};
    for (var i = 0; i < (list ? list.length : 0); i++)
        list[i].cols[0].wins.forEach(function (w) { out[w.address] = true; });
    return out;
}

// The page holding window `address`, or null.
function pageOf(list, address) {
    for (var i = 0; i < (list ? list.length : 0); i++)
        for (var j = 0; j < list[i].cols.length; j++)
            for (var k = 0; k < list[i].cols[j].wins.length; k++)
                if (list[i].cols[j].wins[k].address === address)
                    return list[i];
    return null;
}

// Where the view should be: the page holding the focused window `address` starts at the screen's
// left edge, where a column's window has x `left`. Returns {address (that page's first window),
// x: left} when the view is elsewhere, or null when it already is there or the window isn't on a
// page (the bridge's `scroll` then moves the view by what it reads the gap to be).
function snap(wins, address, room, extra, left, starts) {
    var p = pageOf(pages(wins, room, extra, starts), address);
    if (p === null || Math.abs(left - p.x) <= TOLERANCE)
        return null;
    return { address: p.cols[0].wins[0].address, x: Math.round(left) };
}

// The screen geometry pages need, from a Hyprland monitor (its lastIpcObject: x, width, scale,
// reserved [left, top, right, bottom]) and the window look ({gapLeft, gapRight, windowGap,
// border}): {room (what the columns share), extra (what a column takes besides its window's
// width), left (a column's window x at the screen's left edge)}; null when unreadable.
function geometry(m, look) {
    if (!m || typeof m.width !== "number" || !look)
        return null;
    var scale = m.scale > 0 ? m.scale : 1;
    var res = m.reserved && m.reserved.length >= 4 ? m.reserved : [0, 0, 0, 0];
    var extra = look.windowGap + 2 * look.border;
    return {
        room: m.width / scale - res[0] - res[2] - look.gapLeft - look.gapRight,
        extra: extra,
        left: (m.x || 0) + res[0] + look.gapLeft + extra / 2
    };
}

// The empty part of the page holding window `address`, when something else would show there:
// the page leaves part of the screen unused (a Half column on its own) and isn't the last page
// (after which the strip simply ends). Returns {from} (how far into the page, from its left edge,
// the empty part begins, so it runs from there to the screen's right edge) or null.
function cover(wins, address, room, extra, starts) {
    var list = pages(wins, room, extra, starts), p = pageOf(list, address);
    if (p === null || p === list[list.length - 1] || p.used >= room - TOLERANCE)
        return null;
    return { from: p.used };
}

// The windows to hide: beside a page that leaves part of the screen empty (cover), the next
// page's windows would show; they are hidden whenever their own page isn't the one shown (the
// page holding `address`), so they never show in that space, nor while the view slides past.
// Returns {address: true}.
function hidden(wins, address, room, extra, starts) {
    var list = pages(wins, room, extra, starts), shown = pageOf(list, address), out = {};
    for (var i = 0; i + 1 < list.length; i++) {
        var p = list[i], next = list[i + 1];
        if (p.used >= room - TOLERANCE || next === shown)
            continue;
        next.cols.forEach(function (c) { c.wins.forEach(function (w) { out[w.address] = true; }); });
    }
    return out;
}
