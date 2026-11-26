.pragma library
.import "baritems.js" as Items

// Fitting the bar to a screen: sizes and the cell grid, stepping islands down a size (and
// leaving the most expendable out), placing them, and where a dragged island can drop.
// barlayout.js re-exports these.

var ZONES = Items.ZONES;
var SEPARATOR = Items.SEPARATOR;
var CUSTOM = Items.CUSTOM;
var SIZES = Items.SIZES;
var MIN_CUSTOM = Items.MIN_CUSTOM;
var MAX_CUSTOM = Items.MAX_CUSTOM;
var UNITS = Items.UNITS;
var EXPENDABLE = Items.EXPENDABLE;
var sizeOf = Items.sizeOf;
var unitsOf = Items.unitsOf;
var _cleanUnits = Items._cleanUnits;
var fillOf = Items.fillOf;
var key = Items.key;

// The next size up (dir 1) or down (-1) from `size` (with `units` for Custom), as
// {size, units}, or null past the ends. Past Large it goes Custom, one Icon size at a time.
function stepSize(size, dir, units) {
    if (size === CUSTOM) {
        var u = _cleanUnits(units) + dir;
        if (u > MAX_CUSTOM)
            return null;
        return u < MIN_CUSTOM ? { size: "large", units: null } : { size: CUSTOM, units: u };
    }
    var i = SIZES.indexOf(size) + dir;
    if (i < 0 || i >= SIZES.length)
        return null;
    return SIZES[i] === CUSTOM ? { size: CUSTOM, units: MIN_CUSTOM } : { size: SIZES[i], units: null };
}

// The size nearest `px` wide, for a resize by dragging: {size, units}. Up to Large it snaps to
// the tiers; past Large it is Custom, in whole Icon sizes. With a grid `gr` and metrics `m`,
// widths are as drawn on it (unitWidth); with a number, that is the width of one unit.
function sizeForWidth(px, gr, m) {
    var w = typeof gr === "number" ? function (n) { return n * gr; }
                                   : function (n) { return unitWidth(n, gr, m); };
    var unit = typeof gr === "number" ? gr : m.unit;
    var best = "icon", bestD = Infinity;
    for (var i = 0; i < SIZES.length; i++) {
        if (SIZES[i] === CUSTOM)
            continue;
        var d = Math.abs(w(UNITS[SIZES[i]]) - px);
        if (d < bestD) {
            bestD = d;
            best = SIZES[i];
        }
    }
    var n = Math.round(px / unit), bestN = n;
    // The custom width nearest px as drawn (neighbours may draw alike on a coarse grid).
    for (var k = n - 2; k <= n + 2; k++)
        if (k >= MIN_CUSTOM && Math.abs(w(k) - px) < Math.abs(w(bestN) - px))
            bestN = k;
    n = Math.min(bestN, MAX_CUSTOM);
    if (n >= MIN_CUSTOM && Math.abs(w(n) - px) < bestD)
        return { size: CUSTOM, units: Math.min(n, MAX_CUSTOM) };
    return { size: best, units: null };
}

// Whether {size, units} is what `item` has now.
function sameSize(item, s) {
    return sizeOf(item) === s.size && (s.size !== CUSTOM || unitsOf(item) === s.units);
}


// --- fitting a screen ---
//
// The bar is a grid of equal cells that spans its width exactly, whatever the screen: CELLS
// cells to an Icon island and the gap after it (m.unit + m.gap), so an Icon island is exactly
// CELLS cells and stays square. An island of N units (N Icon sizes, N × m.unit) takes the whole
// number of cells nearest that width (colsFor), gaps inside it included, so every size is drawn
// within half a cell of its width while islands meet the bar's ends and each other on the grid.
// A wider screen gets more cells, not wider ones. A separator takes one cell, with a gap on each
// side. Empty space beside a zone goes to the islands there that fill (fillOf), or stays as whole
// cells. metrics: {unit, gap}.
var CELLS = 4;

// The grid on a bar `width` wide: {cols, pitch} where pitch is a cell's step (cols cells).
function grid(width, m) {
    var g = m.gap || 0;
    var cols = Math.max(CELLS, Math.round((width + g) * CELLS / (m.unit + g)));
    return { cols: cols, pitch: (width + g) / cols };
}

// Where column `c` starts, in whole pixels.
function colX(c, gr) {
    return Math.round(c * gr.pitch);
}

// How wide `units` columns are, gaps between them included.
function spanWidth(units, gr, m) {
    return units > 0 ? colX(units, gr) - (m.gap || 0) : 0;
}

// The columns an island of `units` takes on grid `gr`: those nearest units × m.unit wide.
function colsFor(units, gr, m) {
    if (!(units > 0))
        return 0;
    return Math.max(1, Math.round((units * m.unit + (m.gap || 0)) / gr.pitch));
}

// How wide an island of `units` is drawn on grid `gr`.
function unitWidth(units, gr, m) {
    return spanWidth(colsFor(units, gr, m), gr, m);
}

function _colsOf(id, units, gr, m) {
    return id === SEPARATOR ? 1 : colsFor(units, gr, m);
}

function _rank(id) {
    var i = EXPENDABLE.indexOf(id);
    return i < 0 ? EXPENDABLE.length : i;
}

// How every island is drawn on a bar `width` wide: {sizes: {key: size}, units: {key: width in
// Icon sizes}, hidden: {key: true}, hiddenCount, width (what it all takes)}. While it doesn't
// fit, the biggest island steps down a size (a Custom one an Icon size at a time; the most
// expendable first among equals); once every island is at Icon size, the most expendable are
// left out. The result always fits (an empty bar fits anything).
function fit(bar, width, m) {
    var items = [];
    for (var z = 0; z < ZONES.length; z++) {
        var list = bar[ZONES[z]];
        for (var i = 0; i < list.length; i++)
            items.push({ k: key(ZONES[z], i), zone: ZONES[z], id: list[i].id,
                         size: sizeOf(list[i]), units: unitsOf(list[i]), hidden: false });
    }
    var gr = grid(width, m);
    // Columns taken.
    function total() {
        var sum = 0;
        for (var j = 0; j < items.length; j++)
            if (!items[j].hidden)
                sum += _colsOf(items[j].id, items[j].units, gr, m);
        return sum;
    }
    var guard = 0;
    while (total() > gr.cols && guard++ < 2000) {
        var pick = null;
        for (var a = 0; a < items.length; a++) {
            var it = items[a];
            if (it.hidden || it.id === SEPARATOR || it.size === "icon")
                continue;
            if (pick === null || it.units > pick.units
                    || (it.units === pick.units && _rank(it.id) < _rank(pick.id)))
                pick = it;
        }
        if (pick !== null) {
            var next = stepSize(pick.size, -1, pick.units);
            pick.size = next.size;
            pick.units = next.size === CUSTOM ? next.units : UNITS[next.size];
            continue;
        }
        var drop = null;
        for (var b = items.length - 1; b >= 0; b--) {
            if (items[b].hidden)
                continue;
            if (drop === null || _rank(items[b].id) < _rank(drop.id))
                drop = items[b];
        }
        if (drop === null)
            break;
        drop.hidden = true;
    }
    var out = { sizes: {}, units: {}, hidden: {}, hiddenCount: 0, width: spanWidth(total(), gr, m),
                cols: total(), grid: gr };
    for (var c = 0; c < items.length; c++) {
        out.sizes[items[c].k] = items[c].size;
        out.units[items[c].k] = items[c].units;
        if (items[c].hidden) {
            out.hidden[items[c].k] = true;
            out.hiddenCount++;
        }
    }
    return out;
}

// Where each shown island goes: {key: {x, w, size, units}}, on the grid (see above). The left
// zone from the first cell, the right zone ending at the last, the centre zone centred unless
// that would run into a neighbour, in which case it moves just enough. Then the empty cells on
// each side of the centre zone go to the islands that fill (fillOf): those between the left
// zone and the centre to the left zone's, else the centre's; those between the centre and the
// right zone to the right zone's, else the centre's (with no centre zone, the one gap goes to
// the left zone's, else the right's). Fillers share their side evenly, the first ones getting
// any odd cell. A separator stands between its neighbours: its slot runs from one island's end
// to the next one's start, and it has no width at a zone's end or next to another separator.
// With a fitted layout nothing overlaps.
function place(bar, fitted, width, m) {
    var out = {}, gr = fitted.grid || grid(width, m), g = m.gap || 0;
    function row(zone) {
        var list = bar[zone], cells = [];
        for (var i = 0; i < list.length; i++) {
            var k = key(zone, i);
            if (fitted.hidden[k])
                continue;
            var u = fitted.units && fitted.units[k] !== undefined ? fitted.units[k] : UNITS[fitted.sizes[k]];
            cells.push({ k: k, span: _colsOf(list[i].id, u, gr, m), sep: list[i].id === SEPARATOR,
                         fill: fillOf(list[i]), size: fitted.sizes[k], units: u });
        }
        return cells;
    }
    function spanOf(cells) {
        var n = 0;
        for (var i = 0; i < cells.length; i++)
            n += cells[i].span;
        return n;
    }
    function fillers(cells) {
        return cells.filter(function (c) { return c.fill; });
    }
    // Share `n` cells among `who`.
    function share(who, n) {
        for (var i = 0; i < who.length; i++)
            who[i].span += Math.floor(n / who.length) + (i < n % who.length ? 1 : 0);
    }
    var s = row("start"), c = row("center"), e = row("end");
    var ss = spanOf(s), cs = spanOf(c), es = spanOf(e);
    var lo = ss, hi = gr.cols - es - cs;
    var cc = Math.max(lo, Math.min(Math.round((gr.cols - cs) / 2), hi));
    var fs = fillers(s), fc = fillers(c), fe = fillers(e);
    if (c.length === 0) {
        var gap = gr.cols - ss - es;
        share(fs.length > 0 ? fs : fe, gap);
    } else {
        var left = cc - ss, right = gr.cols - es - cc - cs;
        if (fs.length > 0)
            share(fs, left);
        else if (fc.length > 0) {
            share(fc, left);
            cc -= left;
        }
        if (fe.length > 0)
            share(fe, right);
        else if (fc.length > 0)
            share(fc, right);
    }
    var parts = [[s, 0], [c, cc], [e, gr.cols - spanOf(e)]];
    for (var p = 0; p < parts.length; p++) {
        var cells = parts[p][0], at = parts[p][1];
        for (var j = 0; j < cells.length; j++) {
            var it = cells[j];
            if (!it.sep) {
                // From its first cell to its last, so rounding never opens or closes a gap.
                var x0 = colX(at, gr);
                out[it.k] = { x: x0, w: colX(at + it.span, gr) - g - x0, size: it.size, units: it.units };
            } else {
                var between = j > 0 && j < cells.length - 1 && !cells[j - 1].sep && !cells[j + 1].sep;
                var sx = colX(at, gr) - g;
                out[it.k] = between ? { x: sx, w: colX(at + it.span, gr) - sx, size: it.size, units: it.units }
                                    : { x: Math.max(0, Math.min(colX(at, gr), width)), w: 0, size: it.size, units: it.units };
            }
            at += it.span;
        }
    }
    return out;
}

// Places an island being dragged can be dropped: [{zone, index, x}], index counted in the zone
// as it is now. Between neighbours, at each zone's ends, and an empty zone's usual spot.
function dropPoints(bar, placedMap, width) {
    var pts = [];
    var anchors = { start: 0, center: width / 2, end: width };
    for (var z = 0; z < ZONES.length; z++) {
        var zone = ZONES[z], list = bar[zone], shown = [];
        for (var i = 0; i < list.length; i++) {
            var p = placedMap[key(zone, i)];
            if (p)
                shown.push({ i: i, x: p.x, w: p.w });
        }
        if (shown.length === 0) {
            pts.push({ zone: zone, index: list.length, x: anchors[zone] });
            continue;
        }
        for (var j = 0; j < shown.length; j++)
            pts.push({ zone: zone, index: shown[j].i, x: shown[j].x });
        var last = shown[shown.length - 1];
        pts.push({ zone: zone, index: last.i + 1, x: last.x + last.w });
    }
    return pts;
}

// The drop point nearest `x`.
function nearestDrop(points, x) {
    var best = null, bestD = Infinity;
    for (var i = 0; i < points.length; i++) {
        var d = Math.abs(points[i].x - x);
        if (d < bestD) {
            bestD = d;
            best = points[i];
        }
    }
    return best;
}


// Every shown island in bar order, as [{zone, index, key}], for walking them with the keyboard.
function order(bar, fitted) {
    var out = [];
    for (var z = 0; z < ZONES.length; z++)
        for (var i = 0; i < bar[ZONES[z]].length; i++) {
            var k = key(ZONES[z], i);
            if (!fitted || !fitted.hidden[k])
                out.push({ zone: ZONES[z], index: i, key: k });
        }
    return out;
}
