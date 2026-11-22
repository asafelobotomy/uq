pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import "../core"
import "../session"
import "../logic/shutdowntimer.js" as T

// Quick settings, System tab: "Shut down in…". Preset chips (15 min, 30 min, 1 hour, 2 hours)
// start the timer at once; Custom opens a field that takes "45", "90m", "1h30" or "1:30".
// While a shutdown is pending the chips give way to the countdown, the clock time it fires at,
// and Cancel. The timer itself is ShutdownTimer (session/), so it keeps running with the panel
// closed and the screen locked.
ColumnLayout {
    id: root

    // Set by QuickSettings: the window is open.
    property bool active: false
    property bool custom: false
    property string customError: ""

    spacing: Theme.gapIn * 2

    onActiveChanged: {
        if (!active) {
            custom = false;
            customError = "";
            field.text = "";
        }
    }

    function startCustom() {
        var mins = T.parseCustom(field.text);
        if (mins < 0) {
            customError = "Type a time from 1 minute to 24 hours, like 45, 1h30 or 1:30.";
            return;
        }
        if (ShutdownTimer.start(mins)) {
            custom = false;
            customError = "";
            field.text = "";
        }
    }

    UqText {
        text: "Shut down in…"
        color: Theme.textMuted
        font.bold: true
    }

    // --- pending: countdown and Cancel ---
    Rectangle {
        visible: ShutdownTimer.active
        Layout.fillWidth: true
        implicitHeight: 40
        radius: Theme.radiusControl
        color: Theme.surfaceAlt
        border.color: Theme.accent
        border.width: 1
        Accessible.role: Accessible.StaticText
        Accessible.name: ShutdownTimer.countdown + ", " + ShutdownTimer.at

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Theme.gapIn * 2
            anchors.rightMargin: Theme.gapIn
            spacing: Theme.gapIn * 2

            UqText {
                Layout.preferredWidth: 18
                horizontalAlignment: Text.AlignHCenter
                text: "⏻"
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0
                UqText {
                    Layout.fillWidth: true
                    text: ShutdownTimer.countdown
                }
                UqText {
                    Layout.fillWidth: true
                    text: ShutdownTimer.at
                    color: Theme.textMuted
                    font.pixelSize: Theme.fontSize - 2
                }
            }
            Chip {
                label: "Cancel"
                name: "Cancel scheduled shutdown"
                raised: true
                onChosen: ShutdownTimer.cancel()
            }
        }
    }

    // --- idle: presets and Custom ---
    Flow {
        visible: !ShutdownTimer.active
        Layout.fillWidth: true
        spacing: Theme.gapIn

        Repeater {
            model: T.PRESETS
            delegate: Chip {
                required property var modelData
                label: modelData.label
                name: "Shut down in " + modelData.label
                onChosen: {
                    root.custom = false;
                    ShutdownTimer.start(modelData.minutes);
                }
            }
        }
        Chip {
            label: "Custom"
            name: "Shut down after a custom time"
            current: root.custom
            onChosen: {
                root.custom = !root.custom;
                root.customError = "";
                if (root.custom)
                    Qt.callLater(function () { field.forceActiveFocus(); });
            }
        }
    }

    RowLayout {
        visible: root.custom && !ShutdownTimer.active
        Layout.fillWidth: true
        spacing: Theme.gapIn

        UqInput {
            id: field
            Layout.fillWidth: true
            implicitHeight: 28
            placeholder: "Minutes, or 1h30"
            maximumLength: 12
            Accessible.name: "Shut down after how long"
            onAccepted: root.startCustom()
            onEdited: root.customError = ""
        }
        Chip {
            label: "Start"
            name: "Start the shutdown timer"
            current: true
            onChosen: root.startCustom()
        }
    }

    UqText {
        visible: root.customError !== "" && root.custom && !ShutdownTimer.active
        Layout.fillWidth: true
        text: root.customError
        color: Theme.textMuted
        wrapMode: Text.WordWrap
    }

    // A button sized to its label: accent when `current`, focus ring, Space / Return.
    component Chip: Rectangle {
        id: chip
        property string label: ""
        property string name: label
        property bool current: false
        // On a surfaceAlt background (the countdown row), so it still reads as a button.
        property bool raised: false
        signal chosen()

        implicitWidth: chipLabel.implicitWidth + Theme.gapIn * 4
        implicitHeight: 28
        radius: Theme.radiusControl
        color: current ? Theme.accent
            : (chipMouse.containsMouse || raised ? Theme.overlay : Theme.surfaceAlt)
        Behavior on color { ColorAnimation { duration: Motion.fast } }
        activeFocusOnTab: true
        border.color: Theme.accent
        border.width: activeFocus && !current ? 1 : 0
        Accessible.role: Accessible.Button
        Accessible.name: name
        Accessible.onPressAction: chip.chosen()
        Keys.onSpacePressed: chip.chosen()
        Keys.onReturnPressed: chip.chosen()

        UqText {
            id: chipLabel
            anchors.centerIn: parent
            text: chip.label
            color: chip.current ? Theme.accentText : Theme.text
        }
        MouseArea {
            id: chipMouse
            anchors.fill: parent
            hoverEnabled: true
            onClicked: chip.chosen()
        }
    }
}
