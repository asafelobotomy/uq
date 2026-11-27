.pragma library
.import "baritems.js" as Items
.import "barfit.js" as Fit

// The bar's layout (`shell.bar`): three zones of islands, each `{id, size, units}` where size is
// one of four tiers, in multiples of the Icon size (a square island as tall as the bar): icon 1x,
// small 2x, medium 4x, large 6x; or "custom", wider than large in whole Icon sizes (`units`,
// MIN_CUSTOM to MAX_CUSTOM); absent means the island's own default. Pure helpers, no
// Quickshell imports. uqd validates every write; these only keep the shell from sending what it
// would refuse, give an older uqd (no `bar` yet) the layout its switches meant, and decide how
// the islands fit a screen: never overlapping, never cut off, stepping islands down a size (and
// as a last resort leaving some out) instead.

// From baritems.js.
var ZONES = Items.ZONES;
var SEPARATOR = Items.SEPARATOR;
var MAX_ITEMS = Items.MAX_ITEMS;
var CUSTOM = Items.CUSTOM;
var SIZES = Items.SIZES;
var MIN_CUSTOM = Items.MIN_CUSTOM;
var MAX_CUSTOM = Items.MAX_CUSTOM;
var UNITS = Items.UNITS;
var SIZE_NAMES = Items.SIZE_NAMES;
var MONO = Items.MONO;
var TEXT_MIN = Items.TEXT_MIN;
var TEXT_MAX = Items.TEXT_MAX;
var TEXT_DEFAULT = Items.TEXT_DEFAULT;
var ISLANDS = Items.ISLANDS;
var EXPENDABLE = Items.EXPENDABLE;
var known = Items.known;
var name = Items.name;
var defaultSize = Items.defaultSize;
var sizeOf = Items.sizeOf;
var unitsOf = Items.unitsOf;
var tierOf = Items.tierOf;
var sizeLabel = Items.sizeLabel;
var _cleanUnits = Items._cleanUnits;
var textOf = Items.textOf;
var allText = Items.allText;
var effectiveText = Items.effectiveText;
var withAllText = Items.withAllText;
var isPlainText = Items.isPlainText;
var _storedText = Items._storedText;
var fontFamily = Items.fontFamily;
var stepTextSize = Items.stepTextSize;
var _withText = Items._withText;
var fillOf = Items.fillOf;
var setFill = Items.setFill;
var layout = Items.layout;
var _copy = Items._copy;
var key = Items.key;
var placed = Items.placed;
var count = Items.count;
var addable = Items.addable;
var find = Items.find;
var withOptions = Items.withOptions;

// Move the item at zone/index to toIndex of toZone (counted after taking it out). Null when
// nothing would change or the place doesn't exist.
function moveTo(bar, zone, index, toZone, toIndex) {
    if (ZONES.indexOf(zone) < 0 || ZONES.indexOf(toZone) < 0 || index < 0
            || index >= bar[zone].length)
        return null;
    var out = _copy(bar);
    var item = out[zone].splice(index, 1)[0];
    var to = Math.max(0, Math.min(toIndex, out[toZone].length));
    if (zone === toZone && to === index)
        return null;
    out[toZone].splice(to, 0, item);
    return out;
}

// One step left (dir -1) or right (+1); past a zone's edge it joins the next zone at the near
// end. Null at the far left or right of the bar.
function move(bar, zone, index, dir) {
    var zi = ZONES.indexOf(zone);
    if (zi < 0 || index < 0 || index >= bar[zone].length)
        return null;
    var to = index + dir;
    if (to >= 0 && to < bar[zone].length)
        return moveTo(bar, zone, index, zone, to);
    var nz = zi + dir;
    if (nz < 0 || nz >= ZONES.length)
        return null;
    return moveTo(bar, zone, index, ZONES[nz], dir < 0 ? bar[ZONES[nz]].length : 0);
}

function canMove(bar, zone, index, dir) {
    return move(bar, zone, index, dir) !== null;
}

function remove(bar, zone, index) {
    if (ZONES.indexOf(zone) < 0 || index < 0 || index >= bar[zone].length)
        return null;
    var out = _copy(bar);
    out[zone].splice(index, 1);
    return out;
}

// Add island `id` at `index` of `zone` (the end when index is past it or missing).
function add(bar, id, zone, index) {
    if (!known(id) || ZONES.indexOf(zone) < 0 || count(bar) >= MAX_ITEMS
            || (id !== SEPARATOR && placed(bar, id)))
        return null;
    var out = _copy(bar);
    var list = out[zone];
    var at = typeof index === "number" && index >= 0 && index <= list.length ? index : list.length;
    list.splice(at, 0, { id: id, size: null });
    return out;
}

// Set a size (with `units` for Custom; missing, the item keeps its width or takes the
// narrowest); the island's own default is stored as "no size".
function setSize(bar, zone, index, size, units) {
    if (SIZES.indexOf(size) < 0 || ZONES.indexOf(zone) < 0 || index < 0
            || index >= bar[zone].length)
        return null;
    var out = _copy(bar);
    var old = out[zone][index], id = old.id;
    if (size === CUSTOM)
        out[zone][index] = _withText(old, { id: id, size: CUSTOM,
                             units: _cleanUnits(units !== undefined ? units : old.units) });
    else
        out[zone][index] = _withText(old, { id: id, size: size === defaultSize(id) ? null : size });
    return out;
}

// Set the text style of the item at zone/index: `t` is merged into what it has (so
// {bold: true} changes only that); null puts back the theme's look. Null for a separator or
// a place that doesn't exist.
function setText(bar, zone, index, t) {
    if (ZONES.indexOf(zone) < 0 || index < 0 || index >= bar[zone].length
            || bar[zone][index].id === SEPARATOR)
        return null;
    var out = _copy(bar);
    var old = out[zone][index], next = {};
    for (var k in old)
        if (k !== "text")
            next[k] = old[k];
    // From what the island shows now: its own text, else the all-islands text.
    var cur = old.text && typeof old.text === "object" ? textOf(old) : allText(bar);
    if (t !== null)
        for (var f in TEXT_DEFAULT)
            if (t[f] !== undefined)
                cur[f] = t[f];
    var stored = t === null ? null : _storedText(textOf({ text: cur }));
    if (stored !== null)
        next.text = stored;
    else if (t !== null && !isPlainText(allText(bar)))
        next.text = {};
    out[zone][index] = next;
    return out;
}

// Style `t` (as setText; null for the theme's own) on every island that has a text (not the
// separators). Null when there is none.
function setTextAll(bar, t) {
    var out = bar, changed = false;
    for (var z = 0; z < ZONES.length; z++)
        for (var i = 0; i < bar[ZONES[z]].length; i++) {
            var next = setText(out, ZONES[z], i, t);
            if (next !== null) {
                out = next;
                changed = true;
            }
        }
    return changed ? out : null;
}

// Size `size` (a tier, not Custom) on every island but the separators; null when there is none.
function setSizeAll(bar, size) {
    if (SIZES.indexOf(size) < 0 || size === CUSTOM)
        return null;
    var out = bar, changed = false;
    for (var z = 0; z < ZONES.length; z++)
        for (var i = 0; i < bar[ZONES[z]].length; i++) {
            if (bar[ZONES[z]][i].id === SEPARATOR)
                continue;
            var next = setSize(out, ZONES[z], i, size);
            if (next !== null) {
                out = next;
                changed = true;
            }
        }
    return changed ? out : null;
}

// From barfit.js.
var stepSize = Fit.stepSize;
var sizeForWidth = Fit.sizeForWidth;
var sameSize = Fit.sameSize;
var CELLS = Fit.CELLS;
var grid = Fit.grid;
var colX = Fit.colX;
var spanWidth = Fit.spanWidth;
var colsFor = Fit.colsFor;
var unitWidth = Fit.unitWidth;
var fit = Fit.fit;
var place = Fit.place;
var dropPoints = Fit.dropPoints;
var nearestDrop = Fit.nearestDrop;
var order = Fit.order;

// What SetBarLayout takes: sizes only when set, options as they were.
function toSetting(bar) {
    var z = function (list) {
        return list.map(function (it) {
            if (it.size === CUSTOM)
                return _withText(it, { id: it.id, size: CUSTOM, units: _cleanUnits(it.units) });
            return _withText(it, SIZES.indexOf(it.size) >= 0 ? { id: it.id, size: it.size } : { id: it.id });
        });
    };
    var out = { start: z(bar.start), center: z(bar.center), end: z(bar.end) };
    if (bar.options && Object.keys(bar.options).length > 0)
        out.options = bar.options;
    return out;
}

// The move edit for dropping the item at zone/index on drop point `pt` (whose index is counted
// before the item is taken out). Null when it stays where it is.
function dropEdit(bar, zone, index, pt) {
    if (pt === null)
        return null;
    var to = pt.index;
    if (pt.zone === zone && to > index)
        to--;
    return moveTo(bar, zone, index, pt.zone, to);
}
