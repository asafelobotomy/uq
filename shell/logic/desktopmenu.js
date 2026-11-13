.pragma library

// The desktop menu (a right click on empty desktop space): its entries, where it and its
// submenus go on screen, keyboard stepping, and the id of a new workspace. Pure data in, data
// out; nothing here runs anything. Names from desktop entries are passed through for UqText.

.import "power.js" as P

// Top-level entries. `sub` names a submenu (subEntries); `action` is what DesktopMenu.qml runs.
// The power entries carry the bridge's Power names (power.js).
function entries() {
    return [
        { id: "workspace", label: "Create workspace", detail: "+", icon: "", action: "workspace" },
        { id: "open", label: "Open", sub: "open" },
        { id: "appearance", label: "Appearance", sub: "appearance" },
        { id: "system", label: "System", sub: "system" }
    ];
}

var OPEN_SLOTS = [
    { id: "terminal", label: "Terminal", icon: "utilities-terminal" },
    { id: "files", label: "File Manager", icon: "system-file-manager" },
    { id: "text", label: "Notepad", icon: "accessories-text-editor" }
];

// A DefaultApps reply's {id, name, icon} for `slot`, or null when it isn't a usable one.
function _app(apps, slot) {
    var a = apps && typeof apps === "object" ? apps[slot] : null;
    if (!a || typeof a !== "object" || typeof a.id !== "string" || a.id === "")
        return null;
    return {
        id: a.id,
        name: typeof a.name === "string" && a.name.trim() !== "" ? a.name : "",
        icon: typeof a.icon === "string" ? a.icon : ""
    };
}

// The entries of a submenu.
//   open:       the default terminal, file manager and text editor by their own names and
//               icons (DefaultApps), each disabled with "Not set" when there is none, then
//               uQ Settings.
//   appearance: Change Wallpaper, Light Mode, Dark Mode; the current variant is `checked`, and
//               the two modes are disabled while `themeable` is false (uqd is down).
//   system:     Log Off, Lock, Restart, Shutdown.
function subEntries(sub, apps, variant, themeable) {
    switch (sub) {
    case "open":
        var out = OPEN_SLOTS.map(function (s) {
            var a = _app(apps, s.id);
            if (a === null)
                return { id: s.id, label: s.label, icon: s.icon, detail: "Not set", enabled: false };
            // An app's own name says what it is, so the slot's name follows it, muted.
            var named = a.name !== "" && a.name !== s.label;
            return { id: s.id, label: named ? a.name : s.label, icon: a.icon !== "" ? a.icon : s.icon,
                     action: "launch", appId: a.id, detail: named ? s.label : "" };
        });
        out.push({ id: "settings", label: "uQ Settings", icon: "preferences-desktop", action: "settings" });
        return out;
    case "appearance":
        return [
            { id: "wallpaper", label: "Change Wallpaper", icon: "preferences-desktop-wallpaper", action: "wallpaper" },
            { id: "light", label: "Light Mode", icon: "weather-clear", action: "variant", variant: "light",
              checked: variant === "light", enabled: themeable !== false },
            { id: "dark", label: "Dark Mode", icon: "weather-clear-night", action: "variant", variant: "dark",
              checked: variant === "dark", enabled: themeable !== false }
        ];
    case "system":
        return [
            { id: "logout", label: "Log Off", icon: "system-log-out", action: "power", power: "logout" },
            { id: "lock", label: "Lock", icon: "system-lock-screen", action: "power", power: "lock" },
            { id: "reboot", label: "Restart", icon: "system-reboot", action: "power", power: "reboot",
              detail: P.needsConfirm("reboot") ? "…" : "" },
            { id: "poweroff", label: "Shutdown", icon: "system-shutdown", action: "power", power: "poweroff",
              detail: P.needsConfirm("poweroff") ? "…" : "" }
        ];
    default:
        return [];
    }
}

function enabled(entry) {
    return !!entry && entry.enabled !== false;
}

// The next enabled index from `from` going `step` (+1 / -1), wrapping; `from` -1 starts before
// the first. -1 when nothing is enabled.
function step(list, from, dir) {
    var n = list ? list.length : 0;
    if (n === 0)
        return -1;
    var d = dir < 0 ? -1 : 1;
    var i = Number(from);
    if (!(i >= 0 && i < n))
        i = d > 0 ? -1 : n;
    for (var k = 0; k < n; k++) {
        i = ((i + d) % n + n) % n;
        if (enabled(list[i]))
            return i;
    }
    return -1;
}

function first(list) {
    return step(list, -1, 1);
}

// Top-left of a w×h menu opened at the pointer (px, py) in an areaW×areaH screen: right of and
// below the pointer, flipped to the left or above where it would run off, then kept `margin`
// inside the screen (a menu bigger than the screen sits at the margin).
function place(px, py, w, h, areaW, areaH, margin) {
    var m = Math.max(0, Number(margin) || 0);
    var x = Number(px) || 0, y = Number(py) || 0;
    if (x + w > areaW - m)
        x = x - w;
    if (y + h > areaH - m)
        y = y - h;
    return {
        x: Math.round(Math.max(m, Math.min(x, areaW - m - w))),
        y: Math.round(Math.max(m, Math.min(y, areaH - m - h)))
    };
}

// Top-left of a w×h submenu for the row at rowY (screen coordinates) of a menu at parentX,
// parentW wide: to its right, else to its left, overlapping it by `overlap`; level with the row,
// moved up to stay on screen.
function placeSub(parentX, parentW, rowY, w, h, areaW, areaH, margin, overlap) {
    var m = Math.max(0, Number(margin) || 0);
    var o = Number(overlap) || 0;
    var right = parentX + parentW - o;
    var left = parentX - w + o;
    var x = right + w <= areaW - m ? right : (left >= m ? left : right);
    return {
        x: Math.round(Math.max(m, Math.min(x, areaW - m - w))),
        y: Math.round(Math.max(m, Math.min(rowY, areaH - m - h)))
    };
}

// The lowest workspace id from 1 up that no workspace has (special ones, with ids below 1,
// don't count): a new, empty one.
function newWorkspaceId(ids) {
    var taken = {};
    (ids || []).forEach(function (i) {
        var n = Number(i);
        if (n >= 1 && Math.floor(n) === n)
            taken[n] = true;
    });
    var id = 1;
    while (taken[id])
        id++;
    return id;
}
