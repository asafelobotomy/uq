import QtQuick
import Quickshell
import Quickshell.Hyprland
import "../core"

// A popup under a bar item for widgets from other apps (Phase 4 spec §2.2): a widget's own
// `popup` Component, or the list of widgets that didn't fit under "…". Closes on a click
// outside, on Esc and on lock; never drawn above the lock screen.
PopupWindow {
    id: pop

    property Item anchorItem: null
    property Component content: null
    // Handed to the content's `ctx`, when it has one (a widget's popup).
    property var ctx: null
    property bool open: false

    function show(item, component, context) {
        anchorItem = item;
        content = component;
        ctx = context === undefined ? null : context;
        open = true;
    }

    function hide() {
        open = false;
    }

    anchor.item: anchorItem
    anchor.edges: Edges.Bottom
    anchor.gravity: Edges.Bottom
    visible: open && anchorItem !== null && content !== null && !Bridge.locked
    implicitWidth: Math.max(40, box.implicitWidth)
    implicitHeight: Math.max(20, box.implicitHeight)
    color: "transparent"

    Rectangle {
        id: box
        anchors.fill: parent
        implicitWidth: body.implicitWidth + Theme.gapOut * 2
        implicitHeight: body.implicitHeight + Theme.gapOut * 2
        color: Theme.surface
        radius: Theme.radius
        border.color: Theme.border
        border.width: Theme.borderWidth
        focus: true
        Keys.onEscapePressed: pop.hide()

        Loader {
            id: body
            x: Theme.gapOut
            y: Theme.gapOut
            active: pop.visible
            sourceComponent: pop.content
            onLoaded: {
                if (pop.ctx !== null && item.ctx !== undefined)
                    item.ctx = pop.ctx;
            }
        }
    }

    HyprlandFocusGrab {
        windows: [pop]
        active: pop.visible
        onCleared: pop.hide()
    }

    Connections {
        target: Bridge
        function onLockedChanged() {
            if (Bridge.locked)
                pop.hide();
        }
    }
}
