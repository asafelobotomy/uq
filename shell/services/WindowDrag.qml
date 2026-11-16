pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import "../core"
import "../logic/mousehold.js" as Hold
import "../logic/work.js" as Work

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

    function begin() {
        root.seen = Hold.fresh();
        root.active = true;
        guard.restart();
    }
    function end() {
        guard.stop();
        if (!root.active)
            return;
        root.active = false;
        // Hyprland un-maximized a Full window to lift it, and re-tiled it as a Half column.
        var a = Work.normAddr(root.seen.full);
        if (a !== null)
            Bridge.call("Dispatch", ["column_width", { address: a, width: "full", keep_focus: false }])
                .catch(function (e) { console.warn("uq: drag: " + e); });
        root.seen = Hold.fresh();
        root.ended();
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
