pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import "../core"
import "../services"
import "../logic/edges.js" as Edges
import "../logic/grips.js" as Grips

// Resize grips: the gap between two tiled windows can be dragged, and only the gap moves: the
// windows on either side share what they had, so their far edges stay where they are. Between
// two columns, one widens by what the other gives up, both set at once as fractions of the
// screen with the left one's left edge pinned (the bridge's `columns`: Hyprland's own resize
// doesn't give a column the width it is asked for, and the layout would scroll); between two
// windows stacked in a column, the upper one's height changes and the lower one gets the rest. Under the pointer the gap shows
// a small grip and a resize cursor. Which gaps count is in logic/grips.js.
//
// Each screen has a transparent surface on the Top layer that takes input only over the grips,
// which lie in the gaps, so windows and the rest of the desktop (its right-click menu) work as
// before. (Hyprland doesn't hand the pointer to a Bottom layer surface in the gaps between tiled
// windows, so it has to be above them.) Off while locked.
Scope {
    id: root

    readonly property bool enabled: !Bridge.locked
    // Every window, as logic/edges.js reads them.
    property var windows: []
    property string windowsKey: ""
    // A drag is under way: hold the grips where they are until it ends.
    property bool dragging: false

    function rebuild() {
        var out = [], vals = Hyprland.toplevels.values;
        for (var i = 0; i < vals.length; i++) {
            var w = Edges.fromIpc(vals[i].lastIpcObject, vals[i].title);
            if (w !== null)
                out.push({ address: w.address, ws: w.ws, x: w.x, y: w.y, w: w.w, h: w.h,
                           floating: w.floating, pinned: w.pinned, hidden: w.hidden,
                           fullscreen: w.fullscreen });
        }
        var key = JSON.stringify(out);
        if (key !== root.windowsKey) {
            root.windowsKey = key;
            root.windows = out;
        }
    }
    function refresh() {
        HyprRefresh.request();
    }

    Component.onCompleted: {
        rebuild();
        refresh();
    }
    Timer {
        id: debounce
        interval: 30
        onTriggered: root.rebuild()
    }
    Connections {
        target: Hyprland
        // Focus moves the scrolling layout's view, without an event of its own.
        function onActiveToplevelChanged() { root.refresh(); }
        function onRawEvent(event) {
            switch (event.name) {
            case "openwindow":
            case "closewindow":
            case "movewindowv2":
            case "changefloatingmode":
            case "fullscreen":
            case "pin":
            case "workspacev2":
            case "configreloaded":
                root.refresh();
                break;
            }
        }
    }
    Connections {
        target: Hyprland.toplevels
        function onValuesChanged() { debounce.restart(); }
    }
    Instantiator {
        model: Hyprland.toplevels.values
        delegate: Connections {
            required property var modelData
            target: modelData
            function onLastIpcObjectChanged() { debounce.restart(); }
        }
    }

    Variants {
        model: Quickshell.screens
        delegate: PanelWindow {
            id: win
            required property var modelData
            readonly property var monitor: Hyprland.monitorFor(modelData)
            readonly property int wsId: monitor && monitor.activeWorkspace ? monitor.activeWorkspace.id : 0
            readonly property var live: {
                if (!root.enabled || wsId < 1)
                    return [];
                var d = win.modelData, on = [];
                for (var i = 0; i < root.windows.length; i++) {
                    var w = root.windows[i];
                    if (w.ws === wsId && !w.floating && !w.pinned && !w.hidden)
                        on.push(w);
                }
                // A fullscreen window covers the screen: nothing to drag between.
                for (var j = 0; j < on.length; j++)
                    if (on[j].fullscreen === 2)
                        return [];
                return Grips.grips(on, { x: d.x, y: d.y, width: d.width, height: d.height },
                                   WindowLook.border, WindowLook.windowGap);
            }
            // What is drawn: held still while a drag is under way.
            property var shown: []
            onLiveChanged: if (!root.dragging) shown = live
            Connections {
                target: root
                function onDraggingChanged() { if (!root.dragging) win.shown = win.live; }
            }

            screen: modelData
            visible: shown.length > 0
            color: "transparent"
            WlrLayershell.layer: WlrLayer.Top
            WlrLayershell.namespace: "uq-resize"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            exclusionMode: ExclusionMode.Ignore
            anchors { top: true; bottom: true; left: true; right: true }

            // Input only over the grips.
            mask: Region { id: maskRegion }
            Instantiator {
                id: regions
                model: win.shown.length
                delegate: Region {
                    required property int index
                    readonly property var g: win.shown[index] || null
                    x: g ? g.x : 0
                    y: g ? g.y : 0
                    width: g ? g.w : 0
                    height: g ? g.h : 0
                }
                onObjectAdded: win.syncMask()
                onObjectRemoved: win.syncMask()
            }
            function syncMask() {
                var list = [];
                for (var i = 0; i < regions.count; i++)
                    if (regions.objectAt(i))
                        list.push(regions.objectAt(i));
                maskRegion.regions = list;
            }

            Repeater {
                model: win.shown
                delegate: Item {
                    id: grip
                    required property var modelData
                    readonly property bool col: modelData.kind === "col"
                    // How far the drag has moved the gap, until fresh positions arrive.
                    property real shift: 0
                    x: modelData.x + (col ? shift : 0)
                    y: modelData.y + (col ? 0 : shift)
                    width: modelData.w
                    height: modelData.h

                    Rectangle {
                        anchors.centerIn: parent
                        width: grip.col ? Math.max(2, Math.min(4, parent.width - 4)) : Math.min(40, parent.width - 8)
                        height: grip.col ? Math.min(40, parent.height - 8) : Math.max(2, Math.min(4, parent.height - 4))
                        radius: Math.min(width, height) / 2
                        color: area.pressed ? Theme.accent : Theme.textMuted
                        Behavior on color { ColorAnimation { duration: Motion.fast } }
                        opacity: area.containsMouse || area.pressed ? 1 : 0
                        Behavior on opacity {
                            enabled: !Settings.reduceMotion
                            NumberAnimation { duration: Motion.fast; easing.type: Motion.easing }
                        }
                    }

                    MouseArea {
                        id: area
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: grip.col ? Qt.SplitHCursor : Qt.SplitVCursor
                        property point start
                        property var want: null
                        property string sent: ""
                        Accessible.role: Accessible.Separator
                        Accessible.name: grip.col ? "Resize the columns on either side" : "Resize the windows above and below"

                        onPressed: function (m) {
                            start = mapToItem(null, m.x, m.y);
                            want = null;
                            sent = "";
                            root.dragging = true;
                            // A Full column is maximized as far as Hyprland is concerned, which
                            // would put its width back: un-maximize it first (it keeps its width).
                            if (grip.col)
                                [grip.modelData, grip.modelData.other].forEach(function (w) {
                                    if (w.fullscreen === 1)
                                        Bridge.call("Dispatch", ["fullscreen", { address: w.address, mode: "maximized" }])
                                            .catch(function () {});
                                });
                        }
                        onPositionChanged: function (m) {
                            if (!pressed)
                                return;
                            var p = mapToItem(null, m.x, m.y), dx = p.x - start.x, dy = p.y - start.y;
                            var d = win.modelData;
                            // Columns share the screen less gaps_out on each side; each takes its
                            // window's width plus its borders and gaps_in.
                            want = grip.col
                                ? ["columns", Grips.columnsCall(grip.modelData, dx,
                                       d.width - WindowLook.gapLeft - WindowLook.gapRight,
                                       WindowLook.windowGap + 2 * WindowLook.border)]
                                : ["resize", { windows: Grips.resizes(grip.modelData, dx, dy) }];
                            var size = Grips.dragSize(grip.modelData, dx, dy);
                            grip.shift = grip.col ? size.w - grip.modelData.from.w : size.h - grip.modelData.from.h;
                            if (!sendTimer.running)
                                sendTimer.start();
                        }
                        onReleased: finish()
                        onCanceled: finish()
                        function finish() {
                            sendTimer.stop();
                            send();
                            root.dragging = false;
                            grip.shift = 0;
                            root.refresh();
                        }
                        function send() {
                            var key = JSON.stringify(want);
                            if (!want || key === sent)
                                return;
                            sent = key;
                            Bridge.call("Dispatch", want)
                                .catch(function (e) { console.warn("uq[WIN-001] resize: " + e); });
                        }
                        Timer {
                            id: sendTimer
                            interval: 33
                            onTriggered: area.send()
                        }
                    }
                }
            }
        }
    }
}
