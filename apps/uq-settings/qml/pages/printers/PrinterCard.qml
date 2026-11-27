// One printer: what it is doing, its documents, and what can be done with it.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsCard {
    id: card
    // Passed in from PrintersPage.qml.
    property var page
    property var pm
    property var removeDialog
    property var settingsDialog

    required property var modelData
    required property int index
    readonly property var p: modelData

    title: String(card.p.info)
    iconName: "printer"
    border.width: card.p["default"] === true ? 2 : 1
    border.color: card.p["default"] === true ? Kirigami.Theme.highlightColor : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.08)

    actions: [
        UqLabel {
            visible: card.p["default"] === true
            text: "Default"
            color: Kirigami.Theme.highlightColor
            font.bold: true
        },
        QQC2.ToolButton {
            icon.name: "configure"
            enabled: page.idle
            display: QQC2.AbstractButton.IconOnly
            Accessible.name: "Settings for " + String(card.p.info)
            QQC2.ToolTip.text: "Settings"
            QQC2.ToolTip.visible: hovered
            onClicked: settingsDialog.ask(card.p)
        },
        QQC2.ToolButton {
            icon.name: "edit-delete"
            enabled: page.idle
            display: QQC2.AbstractButton.IconOnly
            Accessible.name: "Remove " + String(card.p.info)
            QQC2.ToolTip.text: "Remove"
            QQC2.ToolTip.visible: hovered
            onClicked: {
                removeDialog.printer = String(card.p.name);
                removeDialog.name = String(card.p.info);
                removeDialog.open();
            }
        }
    ]

    SettingRow {
        settingId: card.index === 0 ? "printers-list" : ""
        highlight: App.highlight
        iconName: page.stateIcon(card.p)
        label: page.statusLine(card.p)
        Layout.fillWidth: true

        QQC2.Button {
            text: "Make default"
            icon.name: "starred-symbolic"
            visible: card.p["default"] !== true
            enabled: page.idle
            onClicked: pm.setDefault(String(card.p.name))
        }
        QQC2.Button {
            text: "Pause"
            icon.name: "media-playback-pause"
            visible: card.p.state !== "paused" && card.p.state !== "stopped" && card.p.accepting !== false
            enabled: page.idle
            onClicked: pm.pause(String(card.p.name))
        }
        QQC2.Button {
            text: "Resume"
            icon.name: "media-playback-start"
            visible: card.p.state === "paused" || card.p.state === "stopped" || card.p.accepting === false
            enabled: page.idle
            onClicked: pm.resume(String(card.p.name))
        }
    }
    // The printer's own words, when it says something.
    UqLabel {
        visible: String(card.p.message || "") !== ""
        text: String(card.p.message || "")
        wrapMode: Text.Wrap
        opacity: 0.8
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.smallSpacing
    }
    SettingRow {
        settingId: card.index === 0 ? "printers-test" : ""
        highlight: App.highlight
        iconName: page.connectionIcon(String(card.p.connection))
        label: [String(card.p.connection), String(card.p.location || ""), String(card.p.model || "")].filter(s => s !== "").join(" · ")
        Layout.fillWidth: true

        QQC2.Button {
            text: "Print a test page"
            icon.name: "document-print"
            enabled: page.idle
            onClicked: pm.testPage(String(card.p.name))
        }
    }

    Kirigami.Separator {
        Layout.fillWidth: true
    }
    SettingRow {
        settingId: card.index === 0 ? "printers-queue" : ""
        highlight: App.highlight
        iconName: "view-list-details"
        label: card.p.jobs.length === 0 ? "Nothing waiting to print" : (card.p.jobs.length === 1 ? "1 document" : card.p.jobs.length + " documents")
        opacity: card.p.jobs.length === 0 ? 0.8 : 1
        Layout.fillWidth: true

        QQC2.Button {
            text: "Cancel all"
            icon.name: "edit-clear-all"
            visible: card.p.jobs.length > 1
            enabled: page.idle
            onClicked: pm.cancelAll(String(card.p.name))
        }
    }
    Repeater {
        model: card.p.jobs
        delegate: SettingRow {
            id: jobRow
            required property var modelData
            readonly property var j: modelData
            iconName: jobRow.j.printing === true ? "document-print" : "text-x-generic"
            label: jobRow.j.mine === true || String(jobRow.j.owner) === "" ? String(jobRow.j.title) : String(jobRow.j.title) + " (" + String(jobRow.j.owner) + ")"
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing

            UqLabel {
                text: String(jobRow.j.state)
                opacity: 0.8
            }
            QQC2.ToolButton {
                icon.name: "dialog-cancel"
                enabled: page.idle
                display: QQC2.AbstractButton.IconOnly
                Accessible.name: "Cancel " + String(jobRow.j.title)
                QQC2.ToolTip.text: "Cancel"
                QQC2.ToolTip.visible: hovered
                onClicked: pm.cancelJob(jobRow.j.id)
            }
        }
    }
}
