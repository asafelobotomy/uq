.pragma library

// The Storage cleanup island's reading of `crumbs scan --json`.

// What crumbs would free if every enabled category were cleaned; the trash is only emptied when
// named, so it isn't counted.
var LEFT_OUT = ["trash"];

function _num(v) {
    return typeof v === "number" && isFinite(v) && v >= 0;
}

// The categories that have something to free, biggest first, as { name, label, size }; null when
// the text isn't a scan.
function parse(text) {
    var d;
    try {
        d = JSON.parse(text);
    } catch (e) {
        return null;
    }
    if (!d || typeof d !== "object" || !d.data || !Array.isArray(d.data.categories))
        return null;
    var out = [];
    d.data.categories.forEach(function (c) {
        if (!c || typeof c.category !== "string" || c.skipped === true || c.error
            || LEFT_OUT.indexOf(c.category) >= 0 || !_num(c.total) || c.total <= 0)
            return;
        out.push({ name: c.category, label: label(c.category), size: c.total });
    });
    out.sort(function (a, b) { return b.size - a.size; });
    return out;
}

function total(rows) {
    var n = 0;
    (rows || []).forEach(function (r) { n += r.size; });
    return n;
}

var LABELS = {
    "cache": "App caches", "thumbnails": "Thumbnails", "temp": "Temporary files",
    "browser-cache": "Browser caches", "dev-cache": "Developer caches",
    "flatpak-unused": "Unused Flatpak runtimes", "appimage-old": "Old AppImages",
    "logs": "Old logs", "aur-build": "AUR build files", "app-cache": "App browser caches",
    "containers": "Container images", "build-output": "Build folders"
};

function label(name) {
    return LABELS[name] || String(name).slice(0, 40);
}
