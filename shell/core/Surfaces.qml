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

    // Emitted for each surface that a transition closed: it must clear its query and results.
    signal wipe(string surface)

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
