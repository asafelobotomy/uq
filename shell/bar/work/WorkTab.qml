// One window's tab.
import QtQuick
import QtQuick.Effects
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import "../../core"
import "../../services"
import "../../notif"
import "../islands"
import "../../logic/minimise.js" as Min
import "../../logic/work.js" as Work
import "../../logic/dock.js" as Dock

Rectangle {
    id: pill
    // Passed in from WorkIsland.qml.
    property var root

    required property var modelData
    required property int index
    // This tab's window, as last read.
    // The placeholder (tabKeys can run ahead of tabInfo for a moment) has every field
    // a binding reads as a bool, so none of them gets undefined.
    readonly property var win: root.tabInfo[modelData]
        || ({ address: modelData, title: "", cls: "", focused: false, minimised: false })
    // Dragging: this tab follows the pointer; the tabs it passes step aside.
    transform: Translate {
        x: root.dragIndex < 0 ? 0
            : pill.index === root.dragIndex ? root.dragDx
            : (pill.index > root.dragIndex && pill.index <= root.dropIndex) ? -(root.dragWidth + root.metrics.gap)
            : (pill.index < root.dragIndex && pill.index >= root.dropIndex) ? root.dragWidth + root.metrics.gap
            : 0
        Behavior on x {
            enabled: !Settings.reduceMotion && pill.index !== root.dragIndex
            NumberAnimation { duration: Motion.base; easing.type: Motion.easing }
        }
    }
    z: pill.index === root.dragIndex ? 1 : 0
    // A minimised window's tab is an icon, greyed, at the end of the queue.
    readonly property real tabW: win.minimised ? root.metrics.minPill
        : (win.focused ? root.fit.focusW : root.fit.pillW)
    readonly property string tier: win.minimised ? "icon" : Work.tabTier(tabW, root.tiers)
    readonly property bool full: root.isFull(win)
    readonly property string name: win.title || win.cls
    Layout.preferredWidth: tabW
    Layout.fillHeight: true
    radius: Theme.radius - 2
    color: win.focused ? Theme.surfaceAlt : (pillHover.hovered ? Theme.overlay : "transparent")
    border.color: win.focused ? Theme.accent : "transparent"
    border.width: win.focused ? 1 : 0
    opacity: win.minimised ? 0.7 : 1
    Accessible.role: Accessible.PageTab
    readonly property bool alert: root.alerts[modelData] === true
    Accessible.name: name + (win.minimised ? ", minimised" : "")
        + (win.focused ? ", focused" : "") + (full ? ", full width" : "")
        + (alert ? ", wants attention" : "")
    Accessible.onPressAction: root.choose(win)
    Behavior on Layout.preferredWidth {
        enabled: !Settings.reduceMotion
        NumberAnimation { duration: Motion.base; easing.type: Motion.easing }
    }

    HoverHandler {
        id: pillHover
    }
    // At icon size the title shows in a tooltip under the pointer.
    Timer {
        id: tipDelay
        interval: 350
        running: pillHover.hovered && pill.tier === "icon" && root.dragIndex < 0
        onTriggered: pill.tipShown = true
    }
    property bool tipShown: false
    Connections {
        target: pillHover
        function onHoveredChanged() { if (!pillHover.hovered) pill.tipShown = false; }
    }
    Loader {
        active: pill.tipShown && pill.tier === "icon" && pill.name !== ""
        sourceComponent: PopupWindow {
            anchor.item: pill
            anchor.edges: Edges.Bottom
            anchor.gravity: Edges.Bottom
            implicitWidth: tipText.implicitWidth + Theme.gapIn * 2
            implicitHeight: tipText.implicitHeight + Theme.gapIn * 2
            color: "transparent"
            visible: true
            Rectangle {
                anchors.fill: parent
                color: Theme.surface
                radius: Theme.radius
                border.color: Theme.border
                border.width: Theme.borderWidth
                UqText {
                    id: tipText
                    anchors.centerIn: parent
                    width: Math.min(implicitWidth, 320)
                    text: pill.name + (pill.win.minimised ? " (minimised)" : "")
                }
            }
        }
    }
    MouseArea {
        id: pillMouse
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
        property real pressX: 0
        property real pressY: 0
        property bool dragged: false
        // Dragged below the bar, onto the screen: dropped there, the window joins the
        // page shown (PageActions); along the bar, the tabs are reordered.
        property bool overScreen: false
        property real screenX: 0
        // Where in the tab it was taken hold of, so the floating copy keeps that spot
        // under the pointer.
        property real holdX: 0
        property real holdY: 0
        cursorShape: dragged && pressed ? Qt.ClosedHandCursor : Qt.ArrowCursor
        onPressed: function (m) {
            var p = mapToItem(null, m.x, m.y);
            pressX = p.x;
            pressY = p.y;
            holdX = m.x;
            holdY = m.y;
            dragged = false;
            overScreen = false;
        }
        onPositionChanged: function (m) {
            if (!(pressedButtons & Qt.LeftButton))
                return;
            var reorder = Work.canDrag(pill.win), drop = Work.canDropOnScreen(pill.win);
            if (!reorder && !drop)
                return;
            var p = mapToItem(null, m.x, m.y), dx = p.x - pressX, dy = p.y - pressY;
            if (!dragged && Math.max(Math.abs(dx), Math.abs(dy)) < Theme.gapIn * 2)
                return;
            if (!dragged) {
                dragged = true;
                if (reorder)
                    root.startDrag(pill.index);
            }
            var bar = pill.QsWindow.window;
            var below = bar !== null && p.y > bar.height + Theme.gapIn;
            overScreen = drop && below;
            screenX = p.x + WindowLook.edgeLeft;
            // Off the bar (or where the tab can't follow), a see-through copy of the
            // tab rides under the pointer; over the screen, where the window would go
            // is outlined (PageActions, drawn by ScreenPages).
            PageActions.dropHint = below || !reorder ? {
                screen: root.screen ? root.screen.name : "", address: pill.win.address,
                x: screenX, y: p.y + WindowLook.gapTop, drop: overScreen,
                holdX: holdX, holdY: holdY, width: pill.width, height: pill.height,
                title: pill.name, icon: root.iconFor(pill.win.cls)
            } : null;
            if (reorder)
                root.moveDrag(overScreen ? 0 : dx);
        }
        onReleased: {
            if (!dragged)
                return;
            PageActions.dropHint = null;
            if (overScreen) {
                root.cancelDrag();
                PageActions.drop(root.screen ? root.screen.name : "", pill.win.address, screenX);
            } else if (root.dragIndex >= 0) {
                root.endDrag();
            }
            overScreen = false;
        }
        onCanceled: {
            PageActions.dropHint = null;
            overScreen = false;
            if (dragged)
                root.cancelDrag();
        }
        onClicked: function (m) {
            if (dragged)
                return;
            if (m.button === Qt.MiddleButton)
                root.act(pill.win, "close");
            else if (m.button === Qt.RightButton)
                root.openMenu(pill, pill.win);
            else
                root.choose(pill.win);
        }
    }
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: pill.tier === "icon" ? 0 : root.tabPad
        anchors.rightMargin: pill.tier === "icon" ? 0 : 2
        spacing: Theme.gapIn
        // Icon size: the icon, centred, turns into the close button under the pointer.
        Item {
            id: iconSlot
            // Focused tab only. The × waits for the pointer to rest on the icon, so passing over a
            // tab doesn't hide which app it is, and a quick click can't close it.
            property bool closeShown: false
            Timer {
                interval: 400
                running: iconClose.containsMouse && pill.tier === "icon" && pill.win.focused && !pill.win.minimised
                onTriggered: iconSlot.closeShown = true
            }
            Connections {
                target: iconClose
                function onContainsMouseChanged() { if (!iconClose.containsMouse) iconSlot.closeShown = false; }
            }
            Layout.preferredWidth: root.iconSize
            Layout.preferredHeight: root.iconSize
            Layout.alignment: Qt.AlignVCenter | (pill.tier === "icon" ? Qt.AlignHCenter : Qt.AlignLeft)
            Layout.fillWidth: pill.tier === "icon"
            Layout.maximumWidth: root.iconSize
            IconImage {
                anchors.fill: parent
                visible: !(pill.tier === "icon" && iconSlot.closeShown)
                implicitSize: root.iconSize
                backer.sourceSize.width: root.iconSize * root.dpr
                backer.sourceSize.height: root.iconSize * root.dpr
                source: root.iconFor(pill.win.cls)
                layer.enabled: pill.win.minimised === true
                layer.effect: MultiEffect {
                    saturation: -1.0
                }
            }
            AlertDot {
                root: pill.root
                shown: pill.alert && !(pill.tier === "icon" && iconSlot.closeShown)
            }
            Rectangle {
                anchors.fill: parent
                visible: pill.tier === "icon" && iconSlot.closeShown
                radius: width / 2
                color: Theme.overlay
                UqText {
                    anchors.centerIn: parent
                    text: "✕"
                }
            }
            MouseArea {
                id: iconClose
                anchors.fill: parent
                // Only the focused tab's icon turns into a close button; an unfocused or minimised
                // tab's icon never does, so clicking it just focuses or restores the window.
                enabled: pill.tier === "icon" && pill.win.focused && !pill.win.minimised
                hoverEnabled: true
                // Before the × shows, a click goes on to the tab (it focuses the window).
                propagateComposedEvents: true
                Accessible.role: Accessible.Button
                Accessible.name: root.closeLabel(pill.win)
                onClicked: mouse => {
                    if (iconSlot.closeShown)
                        root.closePressed(pill, pill.win);
                    else
                        mouse.accepted = false;
                }
            }
        }
        UqText {
            visible: pill.tier !== "icon"
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            text: pill.name
            color: pill.win.focused ? Theme.text : Theme.textMuted
        }
        // The buttons, kept together at the right end, 2 px apart: the widths in
        // root.tiers allow one gap between the icon (or title) and this group, and
        // 2 px between buttons, so a tab at its narrowest still holds them all.
        Item {
            visible: pill.tier !== "icon"
            Layout.fillWidth: pill.tier !== "large"
            Layout.fillHeight: true
            Layout.preferredWidth: buttonRow.implicitWidth
            Layout.minimumWidth: buttonRow.implicitWidth
            Row {
                id: buttonRow
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2
                // Small: whichever the window isn't, under the pointer (its place is kept).
                TabButton {
                    root: pill.root
                    visible: pill.tier === "small" && !pill.win.minimised
                    opacity: pillHover.hovered ? 1 : 0
                    enabled: pillHover.hovered
                    mark: pill.full ? "half" : "full"
                    label: (pill.full ? "Half width: " : "Full width: ") + pill.name
                    onPressed: root.setFull(pill.win, !pill.full)
                }
                TabButton {
                    root: pill.root
                    visible: (pill.tier === "medium" || pill.tier === "large") && !pill.win.minimised
                    mark: "half"
                    label: "Half width: " + pill.name
                    on: !pill.full
                    onPressed: root.setFull(pill.win, false)
                }
                TabButton {
                    root: pill.root
                    visible: (pill.tier === "medium" || pill.tier === "large") && !pill.win.minimised
                    mark: "full"
                    label: "Full width: " + pill.name
                    on: pill.full
                    onPressed: root.setFull(pill.win, true)
                }
                TabButton {
                    root: pill.root
                    id: closeButton
                    visible: pill.tier !== "icon"
                    mark: "close"
                    label: root.closeLabel(pill.win)
                    onPressed: root.closePressed(closeButton, pill.win)
                }
            }
        }
    }
}
