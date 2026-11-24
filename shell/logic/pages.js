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

// A page leaves part of the screen empty: a Half column on its own (or a narrower pair).
function unfilled(p, room) {
    return p.used < room - TOLERANCE;
}

// Whether page `p` (of `list`) sits on the right of the screen, its empty part on the left:
// `right` ({address: true}) holds the first windows of the pages put there (a tab dropped on the
// right half of its own screen). Only a page that leaves part of the screen empty can, and not
// the first (Hyprland doesn't scroll the strip's first column away from the screen's edge).
function onRight(list, p, room, right) {
    return !!right && !!p && p !== list[0] && unfilled(p, room)
        && right[p.cols[0].wins[0].address] === true;
}

// Where the view should be: the page holding the focused window `address` starts at the screen's
// left edge, where a column's window has x `left` (or, put on the right, ends at its right edge).
// Returns {address (that page's first window), x} when the view is elsewhere, or null when it
// already is there or the window isn't on a page (the bridge's `scroll` then moves the view by
// what it reads the gap to be).
function snap(wins, address, room, extra, left, starts, right) {
    var t = target(wins, address, room, extra, left, starts, right);
    return t === null || Math.abs(t.x - t.at) <= TOLERANCE ? null : { address: t.address, x: t.x };
}

// Where the page holding `address` goes, whether or not it is there now: {address (its first
// window), x (that window's x then), at (its x now)}; null when the window isn't on a page.
function target(wins, address, room, extra, left, starts, right) {
    var list = pages(wins, room, extra, starts), p = pageOf(list, address);
    if (p === null)
        return null;
    var x = onRight(list, p, room, right) ? left + room - p.used : left;
    return { address: p.cols[0].wins[0].address, x: Math.round(x), at: p.x };
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
// the page leaves part of the screen unused (a Half column on its own) and isn't at the end of
// the strip on that side (where the strip simply ends). Returns {from, width} (from the page's
// left edge, where the screen's columns begin) or null.
function cover(wins, address, room, extra, starts, right) {
    var list = pages(wins, room, extra, starts), p = pageOf(list, address);
    if (p === null || !unfilled(p, room))
        return null;
    if (onRight(list, p, room, right))
        return { from: 0, width: room - p.used };
    if (p === list[list.length - 1])
        return null;
    return { from: p.used, width: room - p.used };
}

// The windows to hide: beside a page that leaves part of the screen empty (cover), the next
// page's windows would show (the previous page's, for a page on the right); they are hidden
// whenever their own page isn't the one shown (the page holding `address`), so they never show
// in that space, nor while the view slides past. Returns {address: true}.
function hidden(wins, address, room, extra, starts, right) {
    var list = pages(wins, room, extra, starts), shown = pageOf(list, address), out = {};
    for (var i = 0; i < list.length; i++) {
        if (!unfilled(list[i], room))
            continue;
        var n = onRight(list, list[i], room, right) ? list[i - 1] : list[i + 1];
        if (!n || n === shown)
            continue;
        n.cols.forEach(function (c) { c.wins.forEach(function (w) { out[w.address] = true; }); });
    }
    return out;
}

// The windows beginning pages put on the right that still can be ({address: true}), from `right`.
function rightOf(wins, room, extra, starts, right) {
    var list = pages(wins, room, extra, starts), out = {};
    for (var i = 0; i < list.length; i++)
        if (onRight(list, list[i], room, right))
            out[list[i].cols[0].wins[0].address] = true;
    return out;
}

// A tab dropped on the screen: window `dragged` joins the page shown there (the one holding
// `shown`), as half of it. `side` ("left" or "right") is the half of the screen it was dropped
// on. Returns null when nothing would change (or it can't: the window isn't tiled here, or the
// page has more than two columns), else {steps (columns to move it, as the bridge's move_column
// takes them; 0 when it stays), half (windows to make Half), starts (windows that begin a page
// afterwards: address -> true or false), right (page-first windows put on the right or back:
// address -> true or false), area: {from, width} (the part of the page, from its left edge, it
// will take)}. From another page:
//   - a page with one column (Full, or alone at any width): the window takes the half it was
//     dropped on and the column there slides to the other half; both end up Half;
//   - a page of two (equal or not): the window takes the side it was dropped on, and the column
//     there is pushed out to that side onto a page of its own (on the left: before it; on the
//     right: after it); the column that stays is made Half.
// On its own page:
//   - a Half alone moves to the half it was dropped on, the other left empty, with no column
//     moving (not on the first page, which Hyprland keeps at the screen's left edge);
//   - a pair swaps halves when dropped on the other one.
function dropPlan(wins, room, extra, starts, shown, dragged, side, right) {
    var list = pages(wins, room, extra, starts), p = pageOf(list, shown), from = pageOf(list, dragged);
    if (p === null || from === null || p.cols.length > 2)
        return null;
    var cols = Grips.columns(wins);
    function first(c) { return c.wins[0].address; }
    function index(addr) {
        for (var i = 0; i < cols.length; i++)
            for (var j = 0; j < cols[i].wins.length; j++)
                if (cols[i].wins[j].address === addr)
                    return i;
        return -1;
    }
    function slot(c) { return c.w + (extra || 0); }
    var a = p.cols[0], s = {}, r = {}, half = [], area, steps;
    var isRight = onRight(list, p, room, right);
    if (p === from) {
        if (p.cols.length === 1) {
            if (!unfilled(p, room) || (side === "right") === isRight || (side === "right" && p === list[0]))
                return null;
            r[first(a)] = side === "right";
            return { steps: 0, half: [], starts: {}, right: r,
                     area: side === "right" ? { from: room - p.used, width: p.used } : { from: 0, width: p.used } };
        }
        var mine = index(dragged) === index(first(a)) ? 0 : 1;
        var want = side === "left" ? 0 : 1;
        if (mine === want)
            return null;
        var other = p.cols[1 - mine], me = p.cols[mine];
        s[dragged] = want === 0;
        s[first(other)] = want !== 0;
        return { steps: want - mine, half: [], starts: s, right: {},
                 area: want === 0 ? { from: 0, width: slot(me) } : { from: room - slot(me), width: slot(me) } };
    }
    half = [dragged];
    area = { from: room / 2, width: room / 2 };
    var before = false;
    if (p.cols.length === 1) {
        before = side === "left";
        if (Math.abs(slot(a) - room / 2) > TOLERANCE)
            half.unshift(first(a));
        if (isRight)
            r[first(a)] = false;
        if (before)
            area = { from: 0, width: room / 2 };
        s[first(a)] = !before;
        s[dragged] = before;
    } else {
        var b = p.cols[1];
        half.push(first(side === "left" ? b : a));
        s[first(a)] = true;
        s[dragged] = side === "left";
        s[first(b)] = side !== "left";
        if (side === "left")
            area = { from: 0, width: room / 2 };
    }
    // Right after the page's first column, or right before it.
    var ia = index(first(a)), id = index(dragged);
    if (before)
        steps = id > ia ? -(id - ia) : ia - id - 1;
    else
        steps = id > ia ? -(id - ia - 1) : ia - id;
    return { steps: steps, half: half, starts: s, right: r, area: area };
}

// Which side of the page shown (the one holding `shown`) a drop at `x` lands on, `x` measured from
// where the screen's columns begin: on a page of two columns, the side of the gap between them,
// wherever a resize grip left it (the window under the pointer is the one replaced); otherwise
// the side of the screen's middle. "left" or "right".
function sideAt(wins, room, extra, starts, shown, right, x) {
    var list = pages(wins, room, extra, starts), p = pageOf(list, shown);
    if (p !== null && p.cols.length === 2) {
        var off = onRight(list, p, room, right) ? room - p.used : 0;
        return x < off + p.cols[0].w + (extra || 0) ? "left" : "right";
    }
    return x < room / 2 ? "left" : "right";
}

// The page shown with window `address`, when it holds more than one column (windows that share a
// screen are framed together; a window on its own isn't): {from, width} (its columns' span, from
// the left edge where the screen's columns begin, where it sits once the view is on it) or null.
function shared(wins, address, room, extra, starts, right) {
    var list = pages(wins, room, extra, starts), p = pageOf(list, address);
    if (p === null || p.cols.length < 2)
        return null;
    return { from: onRight(list, p, room, right) ? room - p.used : 0, width: p.used };
}

// The index of the column holding window `addr` among the columns of `wins` (left to right), or
// -1; and how many columns there are.
function columnIndex(wins, addr) {
    var cols = Grips.columns(wins);
    for (var i = 0; i < cols.length; i++)
        for (var j = 0; j < cols[i].wins.length; j++)
            if (cols[i].wins[j].address === addr)
                return i;
    return -1;
}
function columnCount(wins) {
    return Grips.columns(wins).length;
}

// Strips at a screen's left and right edges that show the wallpaper over a sliver of the window
// beside the page shown: the gaps at the screen's edge (gaps_out) are wider than the gap between
// two columns, so a few pixels of the neighbouring page's window frame show there. `wins` are the
// workspace's tiled windows and `others` its floating and pinned ones ({x, w} in the layout's
// coordinates, window content without borders), `screenX` and `width` the screen's place and
// width, `bands` {left, right} how wide each strip is (the edge gap before a column's frame), and
// `border` the frame's width. Returns {left, right}: whether each strip is wanted. Not where a
// floating window reaches into the strip (it would be covered).
function edgeStrips(wins, others, screenX, width, bands, border) {
    var b = border || 0, x0 = screenX, x1 = screenX + width;
    var out = { left: false, right: false };
    if (!bands || !(width > 0))
        return out;
    var list = wins || [];
    for (var i = 0; i < list.length; i++) {
        var l = list[i].x - b, r = list[i].x + list[i].w + b;
        if (l < x0 && r > x0)
            out.left = true;
        if (r > x1 && l < x1)
            out.right = true;
    }
    var fl = others || [];
    for (var j = 0; j < fl.length; j++) {
        var fl0 = fl[j].x - b, fr = fl[j].x + fl[j].w + b;
        if (fl0 < x0 + bands.left && fr > x0)
            out.left = false;
        if (fr > x1 - bands.right && fl0 < x1)
            out.right = false;
    }
    return out;
}
