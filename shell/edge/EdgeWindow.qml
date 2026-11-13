pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import Quickshell.Widgets
import "../core"
import "../logic/edges.js" as Edges

// One screen edge's indicator (EdgeIndicators.qml makes two per screen). It exists only while
// there is something on its side, and only on the part of the edge no other screen touches.
// Hidden, it is a transparent strip Theme.borderWidth wide that takes pointer input only there
// and never keyboard focus; reaching it lights the edge and slides out a tab, at the pointer's
// height, with the neighbour's app icon (and its workspace number when it is on another
// workspace). With shell.edge_titles the tab shows the window's title too, except while the
// screen is shared. A click anywhere on it, or each wheel notch, focuses that neighbour.
PanelWindow {
    id: win

    required property var edge        // the EdgeIndicators scope
    required property var display     // the ShellScreen; read instead of `screen`, which the
                                      // window re-announces as it maps (a binding loop)
    required property string side     // "left" | "right"

    readonly property bool left: side === "left"
    readonly property var monitor: display ? Hyprland.monitorFor(display) : null
    readonly property int wsId: monitor && monitor.activeWorkspace ? monitor.activeWorkspace.id : 0
    // The window a wheel step just went to, used as the reference until Hyprland's own focus
    // history catches up (cleared shortly after the last step and when the pointer leaves).
    property string stepRef: ""
    // What is on this side (Edges.neighbour), recomputed after its inputs change rather than as
    // a binding: reading Hyprland's workspace list can update it mid-evaluation (a binding loop).
    property var target: null
    function recompute() {
        var ids = win.edge.monitorWorkspaces(win.monitor ? win.monitor.name : "");
        win.target = Edges.neighbour(win.edge.windows, win.wsId, win.side, ids,
            win.stepRef === "" ? null : win.stepRef);
    }
    function schedule() {
        Qt.callLater(win.recompute);
    }
    onWsIdChanged: schedule()
    onStepRefChanged: schedule()
    onMonitorChanged: schedule()
    Component.onCompleted: schedule()
    Connections {
        target: win.edge
        function onWindowsChanged() { win.schedule(); }
        function onWsSerialChanged() { win.schedule(); }
    }
    readonly property var span: Edges.freeSpan(Quickshell.screens, display, side)
    readonly property bool sharing: Bridge.sharing
    readonly property string targetLabel: Edges.label(target, sharing, side)

    property bool revealed: false
    property real tabY: 0
    property real wheelAcc: 0

    readonly property int stripWidth: Math.max(1, Theme.borderWidth)
    readonly property int iconSize: Theme.fontSize * 2
    readonly property int pad: Theme.gapIn
    readonly property int titleMax: Theme.fontSize * 20

    screen: display
    visible: edge.enabled && target !== null && span !== null
    onVisibleChanged: if (!visible) win.conceal()

    color: "transparent"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "uq-edge"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    // Respect the bar's reserved space (and take none): the bar's corners stay the bar's.
    exclusionMode: ExclusionMode.Normal
    exclusiveZone: 0
    anchors {
        top: true
        bottom: true
        left: win.left
        right: !win.left
    }
    margins {
        top: win.span ? win.span.top : 0
        bottom: win.span ? win.span.bottom : 0
    }
    implicitWidth: win.stripWidth + win.iconSize + win.titleMax + Theme.fontSize * 4 + win.pad * 6

    // Input only on the strip, plus the tab while it is out.
    mask: Region {
        item: strip
        Region { item: tabHit }
    }

    function conceal() {
        win.revealed = false;
        win.stepRef = "";
        win.wheelAcc = 0;
        hideTimer.stop();
    }

    function step() {
        var n = win.target;
        if (stepCooldown.running || !win.edge.go(n))
            return;
        win.stepRef = n.address;
        stepCooldown.restart();
        stepRefTimer.restart();
    }

    Timer {
        id: hideTimer
        interval: 350
        onTriggered: win.conceal()
    }
    Timer {
        id: stepCooldown
        interval: 140
    }
    Timer {
        id: stepRefTimer
        interval: 1200
        onTriggered: win.stepRef = ""
    }

    // The lit edge, shown while the tab is out.
    Rectangle {
        id: strip
        width: win.stripWidth
        height: parent.height
        x: win.left ? 0 : parent.width - width
        color: Theme.accent
        opacity: win.revealed ? 1 : 0
        Behavior on opacity {
            enabled: !Settings.reduceMotion
            NumberAnimation { duration: 120 }
        }
    }

    // The tab's input area: the tab's box while it is out, nothing while hidden.
    Item {
        id: tabHit
        x: win.left ? 0 : parent.width - width
        y: tab.y
        width: win.revealed ? tab.width - tab.radius : 0
        height: win.revealed ? tab.height : 0
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        onEntered: {
            hideTimer.stop();
            if (!win.revealed) {
                win.tabY = Edges.tabTop(area.mouseY, tab.height, area.height);
                win.revealed = true;
                win.edge.refresh();
            }
        }
        onExited: hideTimer.restart()
        onClicked: win.step()
        onWheel: wheel => {
            var dy = wheel.angleDelta.y, dx = wheel.angleDelta.x;
            var r = Edges.wheelSteps(win.wheelAcc, Math.abs(dy) >= Math.abs(dx) ? dy : dx);
            win.wheelAcc = r.acc;
            if (r.steps > 0)
                win.step();
            wheel.accepted = true;
        }
    }

    Rectangle {
        id: tab
        readonly property bool hovered: win.revealed && area.containsMouse
            && area.mouseY >= tab.y && area.mouseY < tab.y + tab.height
            && (win.left ? area.mouseX < tab.x + tab.width : area.mouseX >= tab.x)
        readonly property bool showTitle: win.edge.showTitles && !win.sharing && title.text !== ""

        y: win.tabY
        height: win.iconSize + win.pad * 2
        // The edge-side corners sit off-screen, so the tab looks attached to the edge.
        width: row.implicitWidth + win.pad * 2 + radius + win.stripWidth
        radius: Theme.radius
        x: win.left ? (win.revealed ? -radius : -width) : (win.revealed ? parent.width - width + radius : parent.width)
        color: hovered ? Theme.accent : Theme.surface
        border.color: hovered ? Theme.accent : Theme.border
        border.width: Theme.borderWidth
        Accessible.role: Accessible.Button
        Accessible.name: "Go to " + win.targetLabel

        Behavior on x {
            enabled: !Settings.reduceMotion
            NumberAnimation { duration: 140; easing.type: Easing.OutCubic }
        }
        Behavior on width {
            enabled: !Settings.reduceMotion
            NumberAnimation { duration: 120; easing.type: Easing.OutCubic }
        }

        Row {
            id: row
            anchors.verticalCenter: parent.verticalCenter
            x: win.left ? tab.radius + win.stripWidth + win.pad : win.pad
            spacing: win.pad
            layoutDirection: win.left ? Qt.LeftToRight : Qt.RightToLeft

            IconImage {
                anchors.verticalCenter: parent.verticalCenter
                implicitSize: win.iconSize
                backer.sourceSize.width: win.iconSize * (win.display ? win.display.devicePixelRatio : 1)
                backer.sourceSize.height: win.iconSize * (win.display ? win.display.devicePixelRatio : 1)
                source: win.iconFor(win.target ? win.target.cls : "")
            }
            // Another workspace's window: its workspace number.
            UqText {
                anchors.verticalCenter: parent.verticalCenter
                visible: !!win.target && win.target.workspace > 0
                text: win.target ? String(win.target.workspace) : ""
                font.bold: true
                color: tab.hovered ? Theme.accentText : Theme.textMuted
            }
            UqText {
                id: title
                anchors.verticalCenter: parent.verticalCenter
                visible: tab.showTitle
                width: Math.min(implicitWidth, win.titleMax)
                elide: Text.ElideRight
                text: win.target ? (win.target.title || win.target.cls) : ""
                color: tab.hovered ? Theme.accentText : Theme.text
            }
        }
    }

    function iconFor(cls) {
        if (cls === "")
            return Quickshell.iconPath("application-x-executable");
        var d = DesktopEntries.heuristicLookup(cls);
        var name = d && d.icon ? d.icon : cls.toLowerCase();
        return Quickshell.iconPath(name, true) || Quickshell.iconPath("application-x-executable");
    }
}
