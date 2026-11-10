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
// library can't see the Image enum.)
function fillMode(fit) {
    switch (fit) {
    case "fit": return 1;
    case "center": return 6;
    case "tile": return 3;
    default: return 2;
    }
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
