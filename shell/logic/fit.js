.pragma library
.import "work.js" as Work

// Keep windows on screen (shell/windows/FitWindows.qml, shell.fit_windows): a floating window
// that hangs over the edge of its screen's usable area (the screen minus the bar and other
// reserved space) is moved back inside, and one bigger than that area is shrunk to fit, so all
// of its UI can be seen. Pure, so tests/qml/tst_fit.qml covers it without Hyprland.
//
// The rules:
//   - Only floating windows that are shown and not fullscreen. Tiled windows already get the
//     space their layout gives them.
//   - A window's screen is the one its centre is on; failing that, the one Hyprland says it is
//     on.
//   - A window wholly inside the usable area (to within a pixel) is left alone, even flush
//     against the bar. One that isn't gets a gap (Theme.gapOut) from the edges where it lands.
//   - It keeps its size when that fits; otherwise it takes the whole usable width or height
//     (minus the gaps). It moves as little as it can.

// Pixels a window may stick out before it counts as off screen.
var TOLERANCE = 1;

// A window, from a toplevel's lastIpcObject; null without an address or geometry.
function windowFromIpc(o) {
    if (!o || typeof o !== "object" || !_pair(o.at) || !_pair(o.size))
        return null;
    var addr = Work.normAddr(o.address);
    if (addr === null)
        return null;
    return {
        address: addr,
        x: Number(o.at[0]) || 0,
        y: Number(o.at[1]) || 0,
        w: Number(o.size[0]) || 0,
        h: Number(o.size[1]) || 0,
        floating: o.floating === true,
        hidden: o.hidden === true,
        fullscreen: typeof o.fullscreen === "number" ? o.fullscreen : 0,
        monitor: typeof o.monitor === "number" ? o.monitor : -1
    };
}

// A screen, from a Hyprland monitor's lastIpcObject: its rectangle in layout (logical)
// coordinates and its reserved space [left, top, right, bottom]. Null when it is off or
// incomplete.
function monitorFromIpc(o) {
    if (!o || typeof o !== "object" || typeof o.id !== "number" || o.disabled === true)
        return null;
    var scale = Number(o.scale) > 0 ? Number(o.scale) : 1;
    var w = Number(o.width) / scale, h = Number(o.height) / scale;
    // 1, 3, 5 and 7 turn it on its side.
    if (typeof o.transform === "number" && o.transform % 2 === 1) {
        var t = w;
        w = h;
        h = t;
    }
    if (!(w > 0) || !(h > 0))
        return null;
    var r = _quad(o.reserved);
    return { id: o.id, x: Number(o.x) || 0, y: Number(o.y) || 0, w: w, h: h, reserved: r };
}

function _pair(v) {
    return !!v && typeof v === "object" && v.length >= 2;
}

function _quad(v) {
    if (!v || typeof v !== "object" || !(v.length >= 4))
        return [0, 0, 0, 0];
    var out = [];
    for (var i = 0; i < 4; i++)
        out.push(Math.max(0, Number(v[i]) || 0));
    return out;
}

// The screen window `w` belongs to, from `mons` (monitorFromIpc results), or null.
function monitorFor(w, mons) {
    if (!w || !Array.isArray(mons))
        return null;
    var cx = w.x + w.w / 2, cy = w.y + w.h / 2;
    for (var i = 0; i < mons.length; i++) {
        var m = mons[i];
        if (m && cx >= m.x && cx < m.x + m.w && cy >= m.y && cy < m.y + m.h)
            return m;
    }
    for (var j = 0; j < mons.length; j++)
        if (mons[j] && mons[j].id === w.monitor)
            return mons[j];
    return null;
}

// The usable part of screen `m`, less `gap` on every side.
function usable(m, gap) {
    var r = m.reserved || [0, 0, 0, 0], g = Math.max(0, Number(gap) || 0);
    return {
        x: m.x + r[0] + g,
        y: m.y + r[1] + g,
        w: Math.max(1, m.w - r[0] - r[2] - g * 2),
        h: Math.max(1, m.h - r[1] - r[3] - g * 2)
    };
}

// Where window `w` should go on screen `m`: {x, y, w, h} (whole pixels), or null when it is
// fine where it is (or isn't a window these rules move).
function placement(w, m, gap) {
    if (!w || !m || !w.floating || w.hidden || w.fullscreen > 0 || !(w.w > 0) || !(w.h > 0))
        return null;
    var area = usable(m, 0), t = TOLERANCE;
    if (w.x >= area.x - t && w.y >= area.y - t && w.x + w.w <= area.x + area.w + t
            && w.y + w.h <= area.y + area.h + t)
        return null;
    var room = usable(m, gap);
    var nw = Math.min(w.w, room.w), nh = Math.min(w.h, room.h);
    var nx = Math.max(room.x, Math.min(w.x, room.x + room.w - nw));
    var ny = Math.max(room.y, Math.min(w.y, room.y + room.h - nh));
    return { x: Math.round(nx), y: Math.round(ny), w: Math.round(nw), h: Math.round(nh) };
}

// The window address an event names, for the events that can leave a window off screen
// (openwindow, changefloatingmode, movewindowv2); null for others.
function eventAddress(name, data) {
    switch (name) {
    case "openwindow":
    case "changefloatingmode":
    case "movewindowv2":
        return Work.normAddr(String(data || "").split(",")[0]);
    }
    return null;
}

// Whether an event can change every window's screen (screens added, removed or rearranged).
function screensChanged(name) {
    return name === "monitoraddedv2" || name === "monitorremovedv2" || name === "configreloaded";
}
