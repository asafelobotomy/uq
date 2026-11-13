// One app on the Startup page: its name, where it comes from, whether it starts at login, how
// long after login, and what happened at this login. Names, commands and the lines a failed app
// printed come from other packages' files, the person or the journal, so they are shown only
// through UqLabel (plain text); the switch has no `text:` of its own.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

ColumnLayout {
    id: row

    /// One `apps` row of the Startup snapshot.
    property var app: ({})
    /// Whether a change can be sent now (the page isn't busy).
    property bool idle: true
    property int delayMax: 600
    /// Whether **Open user.lua** opens the file (tests turn it off).
    property bool opensFiles: true
    readonly property string name: String(row.app.name || row.app.id || "")
    readonly property bool added: row.app.source === "added"
    readonly property bool failed: row.app.status === "failed"
    readonly property var logLines: (row.app.log || []).map(l => String(l))
    readonly property string sourceText: {
        switch (row.app.source) {
        case "system":
            return "From the system";
        case "yours":
            return "Yours";
        case "added":
            return "Added in uQ";
        default:
            return "";
        }
    }

    /// The switch was flipped: start (or don't start) the app at the next login.
    signal enabledChosen(bool on)
    /// The delay was changed, in seconds.
    signal delayChosen(int secs)
    /// **Remove** was pressed (added entries only).
    signal removeRequested()

    spacing: 0
    Layout.fillWidth: true
    // New state from uQ: show the saved delay again (a refused change snaps back).
    onAppChanged: {
        if (!settle.running)
            delay.value = Qt.binding(() => Number(row.app.delay || 0));
    }
    Accessible.role: Accessible.Grouping
    Accessible.name: row.name

    RowLayout {
        spacing: Kirigami.Units.largeSpacing
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing

        Kirigami.Icon {
            source: String(row.app.icon || "") !== "" ? String(row.app.icon) : "application-x-executable"
            implicitWidth: Kirigami.Units.iconSizes.medium
            implicitHeight: Kirigami.Units.iconSizes.medium
            Accessible.ignored: true
        }
        ColumnLayout {
            spacing: 0
            Layout.fillWidth: true

            UqLabel {
                objectName: "appName"
                text: row.name
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            UqLabel {
                objectName: "appCommand"
                visible: String(row.app.command_text || "") !== ""
                text: String(row.app.command_text || "")
                font.family: "monospace"
                elide: Text.ElideRight
                opacity: 0.7
                Layout.fillWidth: true
            }
            UqLabel {
                objectName: "appSource"
                text: row.sourceText
                opacity: 0.7
                Layout.fillWidth: true
            }
            SourceLine {
                source: row.app.origin || null
                textName: "appOrigin"
                opensFiles: row.opensFiles
                Layout.fillWidth: true
            }
            UqLabel {
                objectName: "appStatus"
                text: String(row.app.status_text || "")
                color: row.failed ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
        }
        Kirigami.Icon {
            source: "chronometer"
            implicitWidth: Kirigami.Units.iconSizes.small
            implicitHeight: Kirigami.Units.iconSizes.small
            opacity: delay.enabled ? 1 : 0.5
            Accessible.ignored: true
        }
        QQC2.SpinBox {
            id: delay
            objectName: "appDelay"
            from: 0
            to: row.delayMax
            value: Number(row.app.delay || 0)
            editable: true
            enabled: row.idle && row.app.can_toggle === true
            Accessible.name: "Seconds to wait after login"
            Accessible.description: row.name
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            QQC2.ToolTip.text: "Seconds to wait after login"
            onValueModified: settle.restart()
        }
        UqLabel {
            text: "s"
        }
        QQC2.Switch {
            id: onSwitch
            objectName: "appSwitch"
            checked: row.app.enabled === true
            enabled: row.idle && row.app.can_toggle === true
            Accessible.name: "Start at login"
            Accessible.description: row.name
            onToggled: {
                row.enabledChosen(onSwitch.checked);
                onSwitch.checked = Qt.binding(() => row.app.enabled === true);
            }
        }
        QQC2.ToolButton {
            objectName: "removeApp"
            visible: row.added
            enabled: row.idle
            text: "Remove"
            icon.name: "edit-delete"
            display: QQC2.AbstractButton.IconOnly
            Accessible.description: row.name
            onClicked: row.removeRequested()
        }
    }
    // Spin changes are sent 400 ms after the last step; a refused one snaps back with the state.
    Timer {
        id: settle
        interval: 400
        onTriggered: row.delayChosen(delay.value)
    }
    QQC2.ToolButton {
        id: logToggle
        objectName: "logToggle"
        visible: row.failed && row.logLines.length > 0
        text: "What it printed"
        icon.name: checked ? "arrow-down" : "arrow-right"
        checkable: true
        display: QQC2.AbstractButton.TextBesideIcon
        Layout.leftMargin: Kirigami.Units.gridUnit * 2
    }
    ColumnLayout {
        visible: logToggle.visible && logToggle.checked
        spacing: 0
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.gridUnit * 3

        Repeater {
            model: row.logLines
            delegate: UqLabel {
                required property string modelData
                required property int index
                objectName: "logLine" + index
                text: modelData
                font.family: "monospace"
                wrapMode: Text.WrapAnywhere
                Layout.fillWidth: true
            }
        }
    }
}
