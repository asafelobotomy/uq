pragma Singleton
import QtQuick
import "../logic/launcher.js" as L
import "../logic/power.js" as P

// Which launcher surface is open (at most one), on which screen, and with which anchor (the app
// menu's button). State changes go through launcher.js transition(); pure QML, no Quickshell
// imports, so it loads under qmltestrunner.
QtObject {
    id: root

    property string open: "closed"      // "closed" | "spotlight" | "appmenu" | "quicksettings" | "notifcentre" | "powermenu" | "desktopmenu"
    property var screen: null           // ShellScreen, or null for the focused monitor
    property var anchor: null           // Item the app menu hangs from, the desktop menu's {x, y} on its screen, or null
    // The confirm step the power menu opens at ("reboot" | "poweroff", from the app menu's
    // restart and shut-down buttons), or "". Set by toggle(), cleared when the menu closes.
    property string powerConfirm: ""

    // Search generation shared by every launcher surface. It lives here, not on a window, so a
    // reloaded or second surface can't restart it below what the bridge has already seen.
    property int generation: 0

    // Emitted for each surface that a transition closed: it must clear its query and results.
    signal wipe(string surface)
    // Emitted after openWith() opened (or found open) `surface`: it must set its query to `text`.
    signal prefill(string surface, string text)

    function nextGeneration() {
        root.generation = L.nextGeneration(root.generation);
        return root.generation;
    }

    function _apply(event) {
        var prev = root.open;
        var next = L.transition({ open: prev }, event);
        root.open = next.open;
        // Spotlight or the app menu opened: the bridge starts cold activatable providers now,
        // so they can answer the first queries (Phase 4 spec §2.4.1). Never waited on.
        if (next.open !== prev && (next.open === "spotlight" || next.open === "appmenu"))
            Bridge.call("LauncherOpened", []).catch(function () {});
        if (next.open !== "powermenu")
            root.powerConfirm = "";
        if (next.wipe) {
            if (event === "lock") {
                root.wipe("spotlight");
                root.wipe("appmenu");
                root.wipe("quicksettings");
                root.wipe("notifcentre");
                root.wipe("powermenu");
                root.wipe("desktopmenu");
            } else if (prev !== "closed") {
                root.wipe(prev);
            }
        }
        return next;
    }

    // name: "spotlight" | "appmenu" | "quicksettings" | "notifcentre" (the built-in notification
    // centre, toggled by NotifBackend) | "powermenu" | "desktopmenu" (the right-click menu on
    // empty desktop space; `anchor` is the click's {x, y} on `screen`, not an IPC name). initialConfirm (powermenu only): the action
    // whose confirm step it opens at, "reboot" or "poweroff"; anything else opens the plain menu.
    // Ignored while locked.
    function toggle(name, screen, anchor, initialConfirm) {
        if (Bridge.locked)
            return;
        var event = name === "spotlight" ? "toggleSpotlight"
            : (name === "appmenu" ? "toggleAppMenu"
            : (name === "quicksettings" ? "toggleQuickSettings"
            : (name === "notifcentre" ? "toggleNotifCentre"
            : (name === "powermenu" ? "togglePowerMenu"
            : (name === "desktopmenu" ? "toggleDesktopMenu" : "")))));
        if (event === "")
            return;
        if (root.open !== name) {
            // Set before `open` so the surface maps on the right screen (and the power menu
            // reads its confirm step when it opens).
            root.screen = screen === undefined ? null : screen;
            root.anchor = anchor === undefined ? null : anchor;
            if (name === "powermenu")
                root.powerConfirm = P.needsConfirm(initialConfirm) ? initialConfirm : "";
        }
        root._apply(event);
    }

    function close() {
        root._apply("close");
    }

    // Shell IPC (`qs ipc ... call uq open|close|toggle spotlight|appmenu|quicksettings|powermenu`) and the keybinds land
    // here. open only opens, close only closes that surface; unknown verbs or names, and every
    // call while locked, are ignored. "notifcentre" is not an IPC name: its keybind goes through
    // NotifBackend, which picks the backend's own centre. Always the focused monitor, no anchor. Returns "ok" or
    // "ignored" (what `qs ipc call` prints).
    function ipc(verb, name) {
        if (Bridge.locked || (name !== "spotlight" && name !== "appmenu" && name !== "quicksettings"
                              && name !== "powermenu"))
            return "ignored";
        switch (verb) {
        case "open":
            if (root.open === name)
                return "ignored";
            break;
        case "close":
            if (root.open !== name)
                return "ignored";
            root.close();
            return "ok";
        case "toggle":
            break;
        default:
            return "ignored";
        }
        root.toggle(name, null, null);
        return "ok";
    }

    // Open Spotlight with `text` as its query (the emoji bind, `uq:emoji`, opens it at ":").
    // Already open, the query is replaced. Only "spotlight"; ignored while locked. Always the
    // focused monitor, no anchor. Returns "ok" or "ignored".
    function openWith(name, text) {
        if (Bridge.locked || name !== "spotlight")
            return "ignored";
        if (root.open !== name)
            root.toggle(name, null, null);
        if (root.open !== name)
            return "ignored";
        root.prefill(name, String(text));
        return "ok";
    }

    function lock() {
        root._apply("lock");
    }

    property Connections lockConn: Connections {
        target: Bridge
        function onLockedChanged() {
            if (Bridge.locked)
                root.lock();
        }
    }
}
