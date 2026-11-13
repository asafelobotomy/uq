.pragma library

// Tray icons uQ already shows elsewhere in the bar, by StatusNotifierItem id. Pure: the bar
// passes the ids and what is in use, this decides.
//
//   nm-applet         uQ's own network indicator sits beside the tray, so nm-applet's icon was a
//                     second Wi-Fi icon. nm-applet keeps running: it answers NetworkManager's
//                     password prompts (VPNs, enterprise Wi-Fi), which uQ doesn't.
//   org.notify.Panel  noti-fi's icon. While noti-fi is the notification backend it takes the
//                     bell's place in the Notifications island instead of sitting in the tray.
var NM_APPLET = "nm-applet";
var NOTIFI = "org.notify.Panel";

// Whether the tray leaves out the item with this id. ctx: { notifiBell: noti-fi's icon is the bell }.
function hiddenInTray(id, ctx) {
    if (id === NM_APPLET)
        return true;
    return id === NOTIFI && ctx !== null && ctx !== undefined && ctx.notifiBell === true;
}

// The item with this id among the tray's items (any array-like with .length), or null.
function find(items, id) {
    if (!items)
        return null;
    for (var i = 0; i < items.length; i++) {
        var it = items[i];
        if (it && it.id === id)
            return it;
    }
    return null;
}

// Whether the Notifications island shows noti-fi's tray icon in place of the bell: only while
// noti-fi is the backend and its icon is in the tray.
function notifiBell(backend, notifiItem) {
    return backend === "notifi" && notifiItem !== null && notifiItem !== undefined;
}

// The tray items to show, in order: not Passive (an app saying it has nothing to show) and not
// one uQ shows elsewhere (hiddenInTray). `items` is any array-like; ctx as hiddenInTray's.
function shownItems(items, ctx) {
    var out = [];
    for (var i = 0; items && i < items.length; i++) {
        var it = items[i];
        if (it && it.status !== PASSIVE && !hiddenInTray(it.id, ctx))
            out.push(it);
    }
    return out;
}

// Quickshell's Status.Passive, so this file needs no Quickshell import.
var PASSIVE = 0;

// How `count` icons fit in `room` pixels, each `cell` wide with `gap` between: {shown, more},
// where `more` is how many go in the "more" list. When they don't all fit, one cell is kept for
// the button that opens it.
function fit(count, room, cell, gap) {
    var n = Math.max(0, Math.floor((room + gap) / (cell + gap)));
    if (count <= n)
        return { shown: count, more: 0 };
    var shown = Math.max(0, n - 1);
    return { shown: shown, more: count - shown };
}
