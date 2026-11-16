pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import "../logic/mousehold.js" as Hold

// Whether the pointer has hold of a window: a mouse bind (Super+drag to move, Super+right-drag to
// resize) is down. Hyprland lifts a tiled window out of its layout for the whole move, so the
// windows around it reflow, and the handlers that watch windows open, close, float and move
// (ColumnPolicy, ScreenPages, FitWindows) must not answer that as if it were the user's doing: they
// wait on `active`, and look again once `ended` fires, at the windows as they are then.
//
// The events come from two hooks rendered before every mouse bind (see logic/mousehold.js). A
// press with no matching release (the compositor restarted its config in between, say) can't hold
// the shell for good: `active` lets go by itself after `limitMs`.
Singleton {
    id: root

    readonly property int limitMs: 30000

    property bool active: false
    // The hold is over: windows may be rearranged again, and have been left as the pointer left them.
    signal ended()

    // What Hyprland did to the held window meanwhile (Hold.note).
    property var seen: Hold.fresh()

    // While a tiled window is lifted by the move: where the pointer is ({x, y}, the layout's
    // coordinates; null before the first reading), read now and then with `hyprctl cursorpos`.
    property var cursor: null
    // What the screen pages (ScreenPages) take from a move that has ended, once, as {address
    // (the window moved, "" if none), x, y (the pointer where it let go, or null), full (the
    // window that was Full before the move, "" if none)}: null when nothing was moved.
    property var drop: null

    function begin() {
        root.seen = Hold.fresh();
        root.cursor = null;
        root.drop = null;
        root.active = true;
        guard.restart();
    }
    function end() {
        guard.stop();
        if (!root.active)
            return;
        root.active = false;
        // Hyprland un-maximized a Full window to lift it, and re-tiled it as a Half column:
        // the pages put it back (Full), or place it on the page it was dropped on.
        var lifted = Hold.address(root.seen.lifted), full = Hold.address(root.seen.full);
        var at = root.cursor;
        root.drop = lifted === "" && full === "" ? null : {
            address: lifted, full: full, x: at ? at.x : null, y: at ? at.y : null
        };
        root.seen = Hold.fresh();
        root.cursor = null;
        root.ended();
    }

    // The pointer, about every 70 ms, while a lifted window is held.
    property Timer poll: Timer {
        interval: 70
        repeat: true
        running: root.active && root.seen.lifted !== ""
        onTriggered: {
            if (!reader.running)
                reader.running = true;
        }
    }
    property Process reader: Process {
        command: ["hyprctl", "cursorpos", "-j"]
        stdout: StdioCollector {
            onStreamFinished: {
                if (!root.active)
                    return;
                try {
                    var p = JSON.parse(text);
                    if (typeof p.x === "number" && typeof p.y === "number")
                        root.cursor = { x: p.x, y: p.y };
                } catch (e) {
                }
            }
        }
    }

    property Timer guard: Timer {
        interval: root.limitMs
        onTriggered: root.end()
    }

    property Connections events: Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (root.active)
                root.seen = Hold.note(root.seen, event.name, event.data);
            var k = Hold.kind(event.name, event.data);
            if (k === "begin")
                root.begin();
            else if (k === "end")
                root.end();
        }
    }
}
