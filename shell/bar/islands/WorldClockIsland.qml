import QtQuick
import QtQuick.Layouts
import Quickshell
import "../../core"
import "../../logic/ticker.js" as T
import "../../logic/shellprefs.js" as ShellPrefs
import ".."

// World clock (`worldclock`): the time in up to four other zones, `New York 09:41`. `compact`
// shows only the first zone. The popup lists them with the day shift and edits the zones. Qt's
// JavaScript has no time zones, so the bridge sends each zone's UTC offset (kept current
// through daylight-saving changes) and the time is worked out here.
Island {
    id: root

    readonly property var zones: (BarLayout.options.worldclock || {}).zones || []
    readonly property var rows: T.zones(Bridge.worldClock)
    readonly property var visibleZones: size === "compact" ? rows.slice(0, 1) : rows
    readonly property bool h24: Settings.values.shell ? Settings.values.shell.clock_24h !== false : true

    implicitWidth: row.implicitWidth + Theme.gapIn * 2
    Accessible.role: Accessible.Button
    Accessible.name: "World clock: " + rows.map(function (z) {
        return z.label + " " + T.zoneTime(clock.date.getTime(), z.offset, root.h24);
    }).join(", ")

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
    }

    RowLayout {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        spacing: Theme.gapOut

        UqText {
            visible: root.zones.length === 0
            text: "Add a time zone"
            color: Theme.textMuted
        }
        Repeater {
            model: root.visibleZones
            delegate: RowLayout {
                required property var modelData
                spacing: Theme.gapIn
                UqText {
                    text: modelData.label
                    color: Theme.textMuted
                    Layout.maximumWidth: Theme.fontSize * 10
                }
                UqText {
                    text: T.zoneTime(clock.date.getTime(), modelData.offset, root.h24)
                }
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: popup.open && popup.anchorItem === root ? popup.hide() : popup.show(root, details)
    }

    IslandPopup {
        id: popup
    }

    function setZones(list) {
        BarLayout.setOptions("worldclock", { zones: list });
    }

    // Common zones offered before the user types one.
    readonly property var suggested: ["America/New_York", "America/Los_Angeles", "Europe/London",
        "Europe/Berlin", "Asia/Kolkata", "Asia/Shanghai", "Asia/Tokyo", "Australia/Sydney", "UTC"]

    Component {
        id: details
        ColumnLayout {
            id: col
            spacing: Theme.gapIn
            width: Theme.fontSize * 26
            property string addError: ""

            UqText {
                text: "World clock"
                font.bold: true
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
                        visible: zrow.info === null && Bridge.worldClock !== null
                        text: "Not a time zone uQ knows"
                        color: Theme.textMuted
                    }
                    UqText {
                        visible: zrow.info !== null
                        text: zrow.info ? T.zoneTime(clock.date.getTime(), zrow.info.offset, root.h24)
                            + " " + zrow.info.abbrev + " " + T.dayShift(clock.date.getTime(), zrow.info.offset,
                            clock.date.getTimezoneOffset()) : ""
                        color: Theme.textMuted
                    }
                    MenuRow {
                        text: "Remove"
                        Accessible.name: "Remove " + zrow.modelData
                        onActivated: root.setZones(root.zones.filter(function (z) { return z !== zrow.modelData; }))
                    }
                }
            }
            RowLayout {
                Layout.fillWidth: true
                UqInput {
                    id: zoneInput
                    Layout.fillWidth: true
                    placeholder: "Time zone, e.g. Europe/Paris"
                    maximumLength: 64
                    Accessible.name: "Add a time zone"
                    onAccepted: addZone.activated()
                    Component.onCompleted: forceActiveFocus()
                }
                MenuRow {
                    id: addZone
                    text: "Add"
                    onActivated: {
                        var z = zoneInput.text.trim().replace(/ /g, "_");
                        if (!/^(UTC|[A-Za-z][A-Za-z0-9_+\-]*\/[A-Za-z][A-Za-z0-9_+\-]*(\/[A-Za-z][A-Za-z0-9_+\-]*)?)$/.test(z)) {
                            col.addError = "Use a zone name like Europe/Paris";
                        } else if (root.zones.indexOf(z) >= 0 || root.zones.length >= 4) {
                            col.addError = root.zones.length >= 4 ? "At most 4 zones" : "Already added";
                        } else {
                            col.addError = "";
                            root.setZones(root.zones.concat([z]));
                            zoneInput.clear();
                        }
                    }
                }
            }
            UqText {
                visible: col.addError !== ""
                text: col.addError
                color: Theme.textMuted
            }
            Flow {
                Layout.fillWidth: true
                spacing: Theme.gapIn
                visible: root.zones.length < 4
                Repeater {
                    model: root.suggested.filter(function (z) { return root.zones.indexOf(z) < 0; })
                    delegate: MenuRow {
                        required property string modelData
                        text: modelData.split("/").pop().replace(/_/g, " ")
                        Accessible.name: "Add " + modelData
                        onActivated: root.setZones(root.zones.concat([modelData]))
                    }
                }
            }
        }
    }
}
