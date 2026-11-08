pragma Singleton
import QtQuick
import "../logic/launcher.js" as L

// Which launcher surface is open (at most one), on which screen, and with which anchor (the app
// menu's button). State changes go through launcher.js transition(); pure QML, no Quickshell
// imports, so it loads under qmltestrunner.
QtObject {
    id: root

    property string open: "closed"      // "closed" | "spotlight" | "appmenu"
    property var screen: null           // ShellScreen, or null for the focused monitor
    property var anchor: null           // Item the app menu hangs from, or null

    // Search generation shared by every launcher surface. It lives here, not on a window, so a
    // reloaded or second surface can't restart it below what the bridge has already seen.
    property int generation: 0

    // Emitted for each surface that a transition closed: it must clear its query and results.
    signal wipe(string surface)

    function nextGeneration() {
        root.generation = L.nextGeneration(root.generation);
        return root.generation;
    }

    function _apply(event) {
        var prev = root.open;
        var next = L.transition({ open: prev }, event);
        root.open = next.open;
        if (next.wipe) {
            if (event === "lock") {
                root.wipe("spotlight");
                root.wipe("appmenu");
            } else if (prev !== "closed") {
                root.wipe(prev);
            }
        }
        return next;
    }

    // name: "spotlight" | "appmenu". Ignored while locked.
    function toggle(name, screen, anchor) {
        if (Bridge.locked)
            return;
        var event = name === "spotlight" ? "toggleSpotlight"
            : (name === "appmenu" ? "toggleAppMenu" : "");
        if (event === "")
            return;
        if (root.open !== name) {
            // Set before `open` so the surface maps on the right screen.
            root.screen = screen === undefined ? null : screen;
            root.anchor = anchor === undefined ? null : anchor;
        }
        root._apply(event);
    }

    function close() {
        root._apply("close");
    }

    // Shell IPC (`qs ipc ... call uq open|close|toggle spotlight|appmenu`) and the keybinds land
    // here. open only opens, close only closes that surface; unknown verbs or names, and every
    // call while locked, are ignored. Always the focused monitor, no anchor. Returns "ok" or
    // "ignored" (what `qs ipc call` prints).
    function ipc(verb, name) {
        if (Bridge.locked || (name !== "spotlight" && name !== "appmenu"))
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
