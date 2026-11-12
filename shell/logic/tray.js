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
