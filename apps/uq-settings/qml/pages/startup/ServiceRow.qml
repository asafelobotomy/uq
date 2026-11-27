// One background service on Startup › Services: its name and description, whether it is
// running, start or stop and restart, its log, and whether it starts at login. uQ's own are
// locked; a blocked one says so. Names and descriptions come from unit files, so they are shown
// through UqLabel (plain text) only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../../components"

RowLayout {
    id: row

    /// One `services` row of the Services snapshot.
    property var service: ({})
    property bool idle: true
    readonly property string name: String(row.service.name || "")
    readonly property bool running: ["running", "starting"].indexOf(row.service.state) >= 0
    readonly property bool changeable: row.service.locked !== true && row.service.blocked !== true

    signal enabledChosen(bool on)
    /// "start", "stop" or "restart".
    signal actRequested(string action)
    signal logRequested()

    spacing: Kirigami.Units.largeSpacing
    Layout.leftMargin: Kirigami.Units.largeSpacing
    Layout.rightMargin: Kirigami.Units.largeSpacing
    Accessible.role: Accessible.Grouping
    Accessible.name: row.name

    Kirigami.Icon {
        source: row.service.locked === true ? "object-locked" : row.service.state === "failed" ? "dialog-error" : "preferences-system-services"
        implicitWidth: Kirigami.Units.iconSizes.medium
        implicitHeight: Kirigami.Units.iconSizes.medium
        opacity: row.service.blocked === true ? 0.5 : 1
        Accessible.ignored: true
    }
    ColumnLayout {
        spacing: 0
        Layout.fillWidth: true

        UqLabel {
            objectName: "serviceName"
            text: row.name
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "serviceDescription"
            visible: text !== ""
            text: String(row.service.description || "")
            elide: Text.ElideRight
            opacity: 0.7
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "serviceStatus"
            text: row.service.locked === true ? String(row.service.status) + ". uQ needs this one." : row.service.blocked === true ? "Blocked: it can't start until it's unblocked" : String(row.service.status || "")
            color: row.service.state === "failed" ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
            opacity: row.service.state === "failed" ? 1 : 0.7
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
    }
    QQC2.ToolButton {
        objectName: "serviceRun"
        text: "Start or stop now"
        icon.name: row.running ? "media-playback-stop" : "media-playback-start"
        display: QQC2.AbstractButton.IconOnly
        enabled: row.idle && row.changeable && row.service.state !== "stopping"
        Accessible.name: row.running ? "Stop now" : "Start now"
        Accessible.description: row.name
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        QQC2.ToolTip.text: row.running ? "Stop it now" : "Start it now"
        onClicked: row.actRequested(row.running ? "stop" : "start")
    }
    QQC2.ToolButton {
        objectName: "serviceRestart"
        text: "Restart"
        icon.name: "view-refresh"
        display: QQC2.AbstractButton.IconOnly
        enabled: row.idle && row.changeable && row.running
        Accessible.description: row.name
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        QQC2.ToolTip.text: "Restart"
        onClicked: row.actRequested("restart")
    }
    QQC2.ToolButton {
        objectName: "serviceLogButton"
        text: "Recent log"
        icon.name: "view-list-text"
        display: QQC2.AbstractButton.IconOnly
        Accessible.description: row.name
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        QQC2.ToolTip.text: "Recent log"
        onClicked: row.logRequested()
    }
    QQC2.Switch {
        id: loginSwitch
        objectName: "serviceAtLogin"
        // Kept in place (not hidden) for a service that can't be switched, so the rows line up.
        opacity: row.service.switchable === true ? 1 : 0
        enabled: row.idle && row.service.switchable === true
        checked: row.service.on === true
        Accessible.name: "Start at login"
        Accessible.description: row.name
        QQC2.ToolTip.visible: hovered && row.service.switchable === true
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        QQC2.ToolTip.text: "Start at login"
        onToggled: {
            row.enabledChosen(loginSwitch.checked);
            loginSwitch.checked = Qt.binding(() => row.service.on === true);
        }
    }
}
