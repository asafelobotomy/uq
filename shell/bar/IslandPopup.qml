import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import "../core"

// A popup under a bar island (or the bar menu): its own layer surface, placed under the island
// and kept on screen, that takes the keyboard while it is open, so its fields, rows (Up/Down,
// Enter) and Esc work. (A PopupWindow hanging off the bar never gets keys: the bar's layer
// doesn't take the keyboard.) Closes on a click outside, on Esc, on lock, and when another bar
// popup opens; never drawn above the lock screen.
//
// Either `show(item, component, ctx)` a Component (a widget's popup on the Status island), or put the content inside
// as children and call `show(item)`.
PanelWindow {
    id: pop

    property Item anchorItem: null
    property Component content: null
    // Handed to the content's `ctx`, when it has one.
    property var ctx: null
    property bool open: false
    default property alias body: inline.data
    // Where the bar puts its islands: Bar.qml's top margin plus its island height.
    readonly property int barBottom: WindowLook.gapTop + BarLayout.unit

    function show(item, component, context) {
        // Quick settings, the app menu or the notification centre goes first: its focus grab
        // ending after this popup's had started took this one down with it, so a click on an
        // island while one was open closed everything.
        if (Surfaces.open !== "closed")
            Surfaces.close();
        anchorItem = item;
        content = component === undefined ? null : component;
        ctx = context === undefined ? null : context;
        place();
        open = true;
        BarLayout.activePopup = pop;
    }

    function hide() {
        open = false;
        if (BarLayout.activePopup === pop)
            BarLayout.activePopup = null;
    }

    // Left edge under the island, kept on screen. The island's scene position is in its bar
    // window, which sits at Bar.qml's left margin (WindowLook.edgeLeft).
    function place() {
        if (!anchorItem)
            return;
        var p = anchorItem.mapToItem(null, 0, 0);
        var sw = screen ? screen.width : 1280;
        var x = WindowLook.edgeLeft + Math.round(p.x);
        placeX = Math.max(Theme.gapOut, Math.min(x, sw - implicitWidth - Theme.gapOut));
    }

    property int placeX: Theme.gapOut
    readonly property var barWindow: anchorItem && anchorItem.QsWindow ? anchorItem.QsWindow.window : null

    screen: barWindow ? barWindow.screen : null
    visible: open && anchorItem !== null && !Bridge.locked
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.namespace: "uq-bar-popup"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    exclusionMode: ExclusionMode.Ignore
    anchors.top: true
    anchors.left: true
    margins.top: barBottom + Theme.gapIn
    margins.left: placeX
    implicitWidth: Math.max(Theme.fontSize * 4, box.implicitWidth)
    implicitHeight: Math.max(Theme.fontSize * 2, box.implicitHeight)
    onImplicitWidthChanged: place()

    Connections {
        target: BarLayout
        function onActivePopupChanged() {
            if (BarLayout.activePopup !== pop && pop.open)
                pop.open = false;
        }
    }

    UqCard {
        id: box
        anchors.fill: parent
        shown: pop.visible
        implicitWidth: Math.max(body.implicitWidth, inline.childrenRect.width) + Theme.gapOut * 2
        implicitHeight: Math.max(body.implicitHeight, inline.childrenRect.height) + Theme.gapOut * 2
        focus: true
        Keys.onEscapePressed: pop.hide()

        Loader {
            id: body
            x: Theme.gapOut
            y: Theme.gapOut
            active: pop.visible && pop.content !== null
            sourceComponent: pop.content
            onLoaded: {
                if (pop.ctx !== null && "ctx" in item)
                    item.ctx = pop.ctx;
            }
        }
        Item {
            id: inline
            x: Theme.gapOut
            y: Theme.gapOut
            width: childrenRect.width
            height: childrenRect.height
        }
    }

    HyprlandFocusGrab {
        // The bar is included so a click on the same island toggles the popup closed instead of
        // clearing the grab and opening it again; another island's popup replaces this one.
        windows: pop.barWindow ? [pop, pop.barWindow] : [pop]
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
