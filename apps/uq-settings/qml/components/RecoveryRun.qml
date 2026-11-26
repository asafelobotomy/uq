// The Restore page's run card: what Bindle is doing for a Rebase, Factory Restore, Roll back or
// the kept installer, line by line as `bindle` prints it, then how it ended. After a rollback or
// a prepared Factory Restore it offers Restart now. Bindle's lines are UqLabels (plain text).
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "."

SettingsCard {
    id: card

    /// The page's RestoreModel.
    required property var model
    /// `{state: idle|running|done|failed, kind, log, restart}`.
    required property var run
    readonly property string state: String(card.run.state || "idle")
    readonly property var log: card.run.log || []
    readonly property string what: {
        switch (card.run.kind) {
        case "rebase":
            return "Rebase";
        case "restore":
            return "Factory Restore";
        case "rollback":
            return "Roll back";
        case "keep-on":
            return "Keeping the installer";
        case "keep-off":
            return "Removing the kept installer";
        case "cancel":
            return "Cancelling Factory Restore";
        }
        return "Bindle";
    }

    /// Closed by the user once the run ended; a new run shows again.
    property bool dismissed: false

    onStateChanged: if (card.state === "running")
        card.dismissed = false
    objectName: "recoveryRun"
    visible: card.state !== "idle" && card.state !== "" && !card.dismissed
    wide: true
    title: card.state === "running" ? card.what + ": working…" : card.state === "done" ? card.what + ": done" : card.what + ": didn't finish"
    iconName: card.state === "running" ? "chronometer" : card.state === "done" ? "dialog-ok" : "dialog-error"
    help: card.state === "running" ? "Bindle may ask for your password. Keep this window open until it's done." : ""

    actions: [
        QQC2.Button {
            objectName: "restartNow"
            visible: card.state === "done" && card.run.restart === true
            text: "Restart now"
            icon.name: "system-reboot"
            onClicked: card.model.reboot()
        },
        QQC2.Button {
            objectName: "cancelRestore"
            visible: card.state === "done" && card.run.kind === "restore"
            text: "Cancel Factory Restore"
            icon.name: "dialog-cancel"
            onClicked: card.model.recover("cancel", 0)
        },
        QQC2.ToolButton {
            objectName: "closeRun"
            visible: card.state !== "running"
            text: "Close"
            icon.name: "window-close"
            display: QQC2.AbstractButton.IconOnly
            QQC2.ToolTip.text: "Close"
            QQC2.ToolTip.visible: hovered
            onClicked: card.dismissed = true
        }
    ]

    QQC2.BusyIndicator {
        visible: card.state === "running"
        running: visible
        Layout.alignment: Qt.AlignLeft
    }
    UqLabel {
        visible: card.state === "done" && card.run.restart === true
        text: card.run.kind === "restore" ? "The next start opens Bindle's installer. Nothing is erased until its last question." : "The rolled-back system starts next time. The one you have now is kept aside."
        wrapMode: Text.Wrap
        Layout.fillWidth: true
    }

    Rectangle {
        visible: card.log.length > 0
        color: Kirigami.Theme.alternateBackgroundColor
        radius: Kirigami.Units.cornerRadius
        Layout.fillWidth: true
        Layout.preferredHeight: Math.min(logView.contentHeight + Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 14)

        QQC2.ScrollView {
            anchors.fill: parent
            anchors.margins: Kirigami.Units.largeSpacing

            ListView {
                id: logView
                objectName: "recoveryLog"
                model: card.log
                clip: true
                onCountChanged: positionViewAtEnd()
                delegate: UqLabel {
                    required property var modelData

                    width: ListView.view.width
                    text: String(modelData)
                    font.family: "monospace"
                    wrapMode: Text.WrapAnywhere
                }
            }
        }
    }
}
