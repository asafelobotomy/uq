import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets
import "../../core"
import "../../logic/ticker.js" as T
import ".."

// System monitor (`sysmon`). Icon: processor use. Small: a chip icon and processor use. Medium:
// and memory use. Large: and the processor's temperature where a sensor exists. A click shows
// every core and the memory in use. The bridge reads /proc and /sys every 2 s, only while this
// island is placed.
Island {
    id: root

    readonly property var s: Bridge.sysmon
    readonly property bool has: s !== null && s !== undefined
    readonly property int iconSize: Theme.fontSize + 5

    Accessible.role: Accessible.Button
    Accessible.name: has ? "Processor " + T.percent(s.cpu) + ", memory " + T.percent(s.mem)
        + (s.temp_c !== null && s.temp_c !== undefined ? ", " + T.temp(s.temp_c) : "")
        : "System monitor"

    RowLayout {
        anchors.centerIn: parent
        width: Math.min(implicitWidth, root.room)
        spacing: Theme.gapIn

        IconImage {
            visible: !root.isIcon
            implicitSize: root.iconSize
            Layout.preferredWidth: root.iconSize
            Layout.preferredHeight: root.iconSize
            source: Quickshell.iconPath("cpu", "utilities-system-monitor")
        }
        UqText {
            text: root.has ? T.percent(root.s.cpu) : "–"
            font.family: Theme.fontMono
            Layout.minimumWidth: implicitWidth
        }
        UqText {
            visible: root.size === "medium" || root.size === "large"
            text: "RAM " + (root.has ? T.percent(root.s.mem) : "–")
            font.family: Theme.fontMono
            color: Theme.textMuted
            Layout.minimumWidth: implicitWidth
        }
        UqText {
            visible: root.size === "large" && root.has && T.temp(root.s.temp_c) !== ""
            text: root.has ? T.temp(root.s.temp_c) : ""
            font.family: Theme.fontMono
            color: root.has && root.s.temp_c >= 90 ? Theme.warning : Theme.textMuted
            Layout.minimumWidth: implicitWidth
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: popup.open && popup.anchorItem === root ? popup.hide() : popup.show(root, details)
    }

    LazyPopup {
        id: popup
    }

    Component {
        id: details
        ColumnLayout {
            spacing: Theme.gapIn
            width: Theme.fontSize * 24

            UqText {
                text: "Processor " + (root.has ? T.percent(root.s.cpu) : "–")
                    + (root.has && T.temp(root.s.temp_c) !== "" ? " · " + T.temp(root.s.temp_c) : "")
                font.bold: true
            }
            GridLayout {
                columns: 2
                columnSpacing: Theme.gapOut * 2
                rowSpacing: 2
                Layout.fillWidth: true
                Repeater {
                    model: root.has && Array.isArray(root.s.cores) ? root.s.cores.slice(0, 64) : []
                    delegate: RowLayout {
                        required property var modelData
                        required property int index
                        spacing: Theme.gapIn
                        Layout.fillWidth: true
                        UqText {
                            text: String(index + 1)
                            color: Theme.textMuted
                            font.family: Theme.fontMono
                            Layout.preferredWidth: Theme.fontSize * 2
                        }
                        // A bar for the core's load; the number beside it says the same.
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: Math.max(2, Math.round(Theme.fontSize / 2))
                            radius: height / 2
                            color: Theme.surfaceAlt
                            Rectangle {
                                width: parent.width * Math.max(0, Math.min(100, Number(modelData) || 0)) / 100
                                height: parent.height
                                radius: parent.radius
                                color: Theme.accent
                            }
                        }
                        UqText {
                            text: T.percent(modelData)
                            font.family: Theme.fontMono
                            Layout.preferredWidth: Theme.fontSize * 3
                            horizontalAlignment: Text.AlignRight
                        }
                    }
                }
            }
            UqText {
                text: "Memory " + (root.has ? T.percent(root.s.mem) + " · " + T.bytes(root.s.mem_used)
                    + " of " + T.bytes(root.s.mem_total) : "–")
                font.bold: true
            }
            MenuRow {
                id: openMonitor
                Layout.fillWidth: true
                text: "Open a system monitor"
                onActivated: {
                    Bridge.call("Launch", [{ desktop_id: "org.gnome.SystemMonitor.desktop" }]).catch(function () {
                        Bridge.call("Launch", [{ desktop_id: "org.kde.plasma-systemmonitor.desktop" }]).catch(function () {
                            Bridge.call("Launch", [{ desktop_id: "btop.desktop" }]).catch(function () {});
                        });
                    });
                    popup.hide();
                }
                Component.onCompleted: forceActiveFocus()
            }
        }
    }
}
