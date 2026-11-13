import QtQuick
import QtQuick.Layouts
import Quickshell
import "../../core"
import "../../logic/ticker.js" as T
import "../../logic/shellprefs.js" as ShellPrefs
import ".."

// World clock (`worldclock`). Icon: the first zone's hour over its minutes. Small: the first
// zone's time. Medium: its name and time. Large: the first two zones. A click lists every zone
// with the day shift; the zones are chosen in edit mode. Qt's JavaScript has no time zones, so
// the bridge sends each zone's UTC offset (kept current through daylight-saving changes).
Island {
    id: root

    readonly property var zones: (BarLayout.options.worldclock || {}).zones || []
    readonly property var rows: T.zones(Bridge.worldClock)
    readonly property bool h24: Settings.values.shell ? Settings.values.shell.clock_24h !== false : true
    readonly property var first: rows.length > 0 ? rows[0] : null
    readonly property var visibleZones: size === "large" ? rows.slice(0, 2) : rows.slice(0, 1)
    function time(z) {
        return T.zoneTime(clock.date.getTime(), z.offset, root.h24);
    }

    Accessible.role: Accessible.Button
    Accessible.name: "World clock: " + rows.map(function (z) { return z.label + " " + root.time(z); }).join(", ")

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
    }

    // Icon: two short lines, so they stay readable in a square.
    Column {
        visible: root.isIcon && root.first !== null
        anchors.centerIn: parent
        spacing: -2
        UqText {
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.first ? root.time(root.first).split(":")[0] : ""
            font.bold: true
        }
        UqText {
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.first ? root.time(root.first).split(":")[1].slice(0, 2) : ""
            color: Theme.textMuted
        }
    }

    UqText {
        visible: !root.isIcon && root.zones.length === 0
        anchors.fill: parent
        verticalAlignment: Text.AlignVCenter
        text: "No zones"
        color: Theme.textMuted
    }

    RowLayout {
        visible: !root.isIcon && root.zones.length > 0
        anchors.centerIn: parent
        width: Math.min(implicitWidth, root.room)
        spacing: Theme.gapOut

        Repeater {
            model: root.isIcon ? [] : root.visibleZones
            delegate: RowLayout {
                required property var modelData
                spacing: Theme.gapIn
                Layout.fillWidth: true
                UqText {
                    visible: root.size !== "small"
                    text: modelData.label
                    color: Theme.textMuted
                    Layout.fillWidth: true
                }
                UqText {
                    text: root.time(modelData)
                    Layout.minimumWidth: implicitWidth
                }
            }
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
                text: "World clock"
                font.bold: true
            }
            UqText {
                visible: root.zones.length === 0
                text: "No time zones chosen yet."
                color: Theme.textMuted
            }
            Repeater {
                model: root.zones
                delegate: RowLayout {
                    id: zrow
                    required property string modelData
                    readonly property var info: {
                        for (var i = 0; i < root.rows.length; i++)
                            if (root.rows[i].zone === modelData)
                                return root.rows[i];
                        return null;
                    }
                    Layout.fillWidth: true
                    UqText {
                        text: zrow.info ? zrow.info.label : zrow.modelData
                        Layout.fillWidth: true
                    }
                    UqText {
                        text: zrow.info ? root.time(zrow.info) + " " + zrow.info.abbrev + " "
                            + T.dayShift(clock.date.getTime(), zrow.info.offset, clock.date.getTimezoneOffset())
                            : (Bridge.worldClock !== null ? "Not a time zone uQ knows" : "")
                        color: Theme.textMuted
                    }
                }
            }
            MenuRow {
                Layout.fillWidth: true
                text: "Change the time zones…"
                onActivated: {
                    popup.hide();
                    root.editRequested();
                }
                Component.onCompleted: forceActiveFocus()
            }
        }
    }
}
