pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
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

    function begin() {
        root.active = true;
        guard.restart();
    }
    function end() {
        guard.stop();
        if (!root.active)
            return;
        root.active = false;
        root.ended();
    }

    property Timer guard: Timer {
        interval: root.limitMs
        onTriggered: root.end()
    }

    property Connections events: Connections {
        target: Hyprland
        function onRawEvent(event) {
            var k = Hold.kind(event.name, event.data);
            if (k === "begin")
                root.begin();
            else if (k === "end")
                root.end();
        }
    }
}
