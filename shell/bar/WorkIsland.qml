import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import "../core"
import "../logic/work.js" as Work

// Focused window: icon, title (untrusted, elided to 40% of the bar width), action buttons and a
// dropdown. With no focused window it shows "Desktop" and nothing else.
Island {
    id: root
    property var screen: null
    property real barWidth: screen ? screen.width : 1280
    readonly property real dpr: screen ? screen.devicePixelRatio : 1
    readonly property int iconSize: 20
    readonly property int cell: 26

    readonly property var tl: Hyprland.activeToplevel
    readonly property bool hasWindow: tl !== null && tl !== undefined
    readonly property string addr: hasWindow ? (Work.normAddr(tl.address) || "") : ""
    readonly property bool actionable: addr !== ""
    readonly property var ipc: hasWindow && tl.lastIpcObject ? tl.lastIpcObject : ({})
    readonly property string cls: ipc["class"] ? String(ipc["class"]) : ""
    readonly property var fromWs: hasWindow ? Work.minimiseFrom(tl.workspace) : null
    readonly property bool isFloating: ipc["floating"] === true
    readonly property bool isPinned: ipc["pinned"] === true

    implicitWidth: row.implicitWidth + Theme.gapIn * 2

    function iconFor(cls) {
        if (cls === "")
            return Quickshell.iconPath("application-x-executable");
        var d = DesktopEntries.heuristicLookup(cls);
        var name = d && d.icon ? d.icon : cls.toLowerCase();
        return Quickshell.iconPath(name, true) || Quickshell.iconPath("application-x-executable");
    }
    function dispatch(name, args) {
        if (!actionable)
            return;
        var a = Object.assign({ address: addr }, args || ({}));
        Bridge.call("Dispatch", [name, a]);
    }

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "activewindowv2" || event.name === "windowtitlev2"
                    || event.name === "changefloatingmode" || event.name === "pin")
                refreshTimer.restart();
        }
    }
    Timer {
        id: refreshTimer
        interval: 60
        onTriggered: Hyprland.refreshToplevels()
    }

    RowLayout {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        spacing: Theme.gapIn

        IconImage {
            visible: root.hasWindow
            implicitSize: root.iconSize
            Layout.preferredWidth: root.iconSize
            Layout.preferredHeight: root.iconSize
            backer.sourceSize.width: root.iconSize * root.dpr
            backer.sourceSize.height: root.iconSize * root.dpr
            source: root.hasWindow ? root.iconFor(root.cls) : ""
        }

        UqText {
            id: titleText
            Layout.maximumWidth: root.barWidth * 0.4
            elide: Text.ElideRight
            text: root.hasWindow ? (root.tl.title || root.cls) : "Desktop"
            color: root.hasWindow ? Theme.text : Theme.textMuted
        }

        Repeater {
            model: root.hasWindow && root.actionable ? [
                { glyph: "▾", tip: "menu", kind: "menu" },
                { glyph: "–", tip: "Minimise", kind: "min" },
                { glyph: "□", tip: "Maximise", kind: "max" },
                { glyph: "✕", tip: "Close", kind: "close" }
            ].filter(function (b) { return b.kind !== "min" || root.fromWs !== null; }) : []
            delegate: Rectangle {
                id: btn
                required property var modelData
                Layout.preferredWidth: root.cell
                Layout.preferredHeight: root.cell
                radius: Theme.radius - 2
                color: btnMouse.containsMouse ? Theme.surfaceAlt : "transparent"
                UqText {
                    anchors.centerIn: parent
                    text: btn.modelData.glyph
                    color: Theme.text
                }
                MouseArea {
                    id: btnMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: {
                        switch (btn.modelData.kind) {
                        case "menu":
                            menu.anchor.item = btn;
                            menu.visible = !menu.visible;
                            break;
                        case "min":
                            root.dispatch("minimise", { from_ws: root.fromWs });
                            break;
                        case "max":
                            root.dispatch("fullscreen", { mode: "maximized" });
                            break;
                        case "close":
                            root.dispatch("close");
                            break;
                        }
                    }
                }
            }
        }
    }

    PopupWindow {
        id: menu
        anchor.window: root.QsWindow.window
        anchor.edges: Edges.Bottom
        anchor.gravity: Edges.Bottom
        implicitWidth: 200
        implicitHeight: col.implicitHeight + Theme.gapIn * 2
        color: "transparent"
        visible: false

        Rectangle {
            anchors.fill: parent
            color: Theme.surface
            radius: Theme.radius
            border.color: Theme.border
            border.width: Theme.borderWidth

            ColumnLayout {
                id: col
                anchors.fill: parent
                anchors.margins: Theme.gapIn
                spacing: 2

                Repeater {
                    model: [
                        { label: root.isFloating ? "Tile" : "Float", act: "toggle_float", args: {} },
                        { label: root.isPinned ? "Unpin" : "Pin", act: "pin", args: {} },
                        { label: "Fullscreen", act: "fullscreen", args: { mode: "full" } }
                    ]
                    delegate: Rectangle {
                        id: item
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.preferredHeight: root.cell
                        radius: Theme.radius - 2
                        color: itemMouse.containsMouse ? Theme.surfaceAlt : "transparent"
                        UqText {
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: parent.left
                            anchors.leftMargin: Theme.gapIn
                            text: item.modelData.label
                        }
                        MouseArea {
                            id: itemMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: {
                                root.dispatch(item.modelData.act, item.modelData.args);
                                menu.visible = false;
                            }
                        }
                    }
                }

                UqText {
                    text: "Move to workspace"
                    color: Theme.textMuted
                    Layout.leftMargin: Theme.gapIn
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 2
                    Repeater {
                        model: Work.wsChoices()
                        delegate: Rectangle {
                            id: wsBtn
                            required property int modelData
                            width: 34
                            height: root.cell
                            radius: Theme.radius - 2
                            color: wsMouse.containsMouse ? Theme.surfaceAlt : "transparent"
                            UqText {
                                anchors.centerIn: parent
                                text: String(wsBtn.modelData)
                            }
                            MouseArea {
                                id: wsMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                onClicked: {
                                    root.dispatch("move", { workspace: wsBtn.modelData });
                                    menu.visible = false;
                                }
                            }
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: root.cell
                    radius: Theme.radius - 2
                    color: closeMouse.containsMouse ? Theme.surfaceAlt : "transparent"
                    UqText {
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        anchors.leftMargin: Theme.gapIn
                        text: "Close"
                    }
                    MouseArea {
                        id: closeMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: {
                            root.dispatch("close");
                            menu.visible = false;
                        }
                    }
                }
            }
        }
    }
    HyprlandFocusGrab {
        windows: [menu]
        active: menu.visible
        onCleared: menu.visible = false
    }
}
