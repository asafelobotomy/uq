.pragma library

// Wallpaper helpers (spec §2.3). Pure: Wallpaper.qml owns the images, folder model and timer.

var DEFAULT_INTERVAL = 300;

// The entry for this output, else the "*" entry, else null.
function entryFor(wallpaper, outputName) {
    var o = wallpaper && wallpaper.outputs;
    if (!o || typeof o !== "object")
        return null;
    if (Object.prototype.hasOwnProperty.call(o, outputName) && o[outputName])
        return o[outputName];
    if (Object.prototype.hasOwnProperty.call(o, "*") && o["*"])
        return o["*"];
    return null;
}

// File names whose extension is in `readableFormats` (case-insensitive), in natural name order.
function slides(fileNames, readableFormats) {
    var out = [];
    for (var i = 0; i < fileNames.length; i++) {
        var n = String(fileNames[i]);
        var dot = n.lastIndexOf(".");
        if (dot < 0)
            continue;
        if (readableFormats.indexOf(n.substring(dot + 1).toLowerCase()) >= 0)
            out.push(n);
    }
    out.sort(naturalCompare);
    return out;
}

// localeCompare with {numeric: true}. QML's JS engine ignores that option, so digit runs are
// compared as numbers here and the rest with localeCompare.
function naturalCompare(a, b) {
    var ca = a.match(/\d+|\D+/g) || [];
    var cb = b.match(/\d+|\D+/g) || [];
    for (var i = 0; i < ca.length && i < cb.length; i++) {
        var x = ca[i], y = cb[i];
        var r;
        if (/^\d/.test(x) && /^\d/.test(y))
            r = Number(x) - Number(y);
        else
            r = x.localeCompare(y);
        if (r !== 0)
            return r < 0 ? -1 : 1;
    }
    return ca.length - cb.length;
}

function nextIndex(i, n) {
    return n > 0 ? (i + 1) % n : -1;
}

// The next index after i, wrapping, that is not in `errored` (an object keyed by index); -1 when
// none is left. Can return i itself when it is the only readable slide.
function nextReadable(i, n, errored) {
    for (var step = 1; step <= n; step++) {
        var j = (i + step) % n;
        if (!(errored && errored[j]))
            return j;
    }
    return -1;
}

// Image.fillMode values: PreserveAspectCrop 2, PreserveAspectFit 1, Pad 6, Tile 3. (A pragma
// library can't see the Image enum.) "span" crops too: the image is sized to the span box.
function fillMode(fit) {
    switch (fit) {
    case "fit": return 1;
    case "center": return 6;
    case "tile": return 3;
    default: return 2;
    }
}

// --- one wallpaper across every screen ("span", spec R24) ---

// The names of the screens that draw one picture together: when the "*" entry's fit is "span",
// every screen without an entry of its own; otherwise none.
function spanNames(wallpaper, screens) {
    var o = wallpaper && wallpaper.outputs;
    var star = o && typeof o === "object" && Object.prototype.hasOwnProperty.call(o, "*") ? o["*"] : null;
    var names = [];
    if (!star || star.fit !== "span" || !screens)
        return names;
    for (var i = 0; i < screens.length; i++) {
        var n = screens[i].name;
        if (!(Object.prototype.hasOwnProperty.call(o, n) && o[n]))
            names.push(n);
    }
    return names;
}

// The box the picture covers, in this screen's own coordinates: the bounding box of the spanning
// screens (their logical x, y, width and height), moved by minus this screen's position. null
// when this screen isn't one of them.
function spanGeometry(screens, name, spanNames) {
    if (!screens || !spanNames || spanNames.indexOf(name) < 0)
        return null;
    var mine = null, left = 0, top = 0, right = 0, bottom = 0, any = false;
    for (var i = 0; i < screens.length; i++) {
        var s = screens[i];
        if (spanNames.indexOf(s.name) < 0)
            continue;
        if (s.name === name)
            mine = s;
        if (!any) {
            left = s.x; top = s.y; right = s.x + s.width; bottom = s.y + s.height;
            any = true;
        } else {
            left = Math.min(left, s.x);
            top = Math.min(top, s.y);
            right = Math.max(right, s.x + s.width);
            bottom = Math.max(bottom, s.y + s.height);
        }
    }
    if (!mine)
        return null;
    return { x: left - mine.x, y: top - mine.y, width: right - left, height: bottom - top };
}

// The slide every spanning screen shows at `nowMs`: floor(now / interval) mod n, moved forward
// past slides that failed to load (`errored` is keyed by index). -1 when there are no slides or
// none is readable.
function clockIndex(nowMs, intervalS, n, errored) {
    if (!(n > 0) || !(intervalS > 0))
        return -1;
    var i = Math.floor(nowMs / (intervalS * 1000)) % n;
    if (!(errored && errored[i]))
        return i;
    var j = nextReadable(i, n, errored);
    return j === i && errored && errored[j] ? -1 : j;
}

// Milliseconds from `nowMs` to the next interval boundary (at least 1).
function msToBoundary(nowMs, intervalS) {
    var step = intervalS * 1000;
    return Math.max(1, step - (nowMs % step));
}

function fallbackColor(entry, themeBg) {
    var c = entry && entry.color;
    return (typeof c === "string" && /^#[0-9a-fA-F]{6}$/.test(c)) ? c : themeBg;
}

function intervalMs(entry) {
    var s = entry && entry.interval;
    return (typeof s === "number" && s > 0 ? s : DEFAULT_INTERVAL) * 1000;
}

// A file:// URL for an absolute local path; encodeURIComponent per segment so # ? % are escaped.
function fileUrl(path) {
    return "file://" + String(path).split("/").map(encodeURIComponent).join("/");
}
