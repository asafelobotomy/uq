// Welcome, the last step: where Settings is, the keys worth knowing, and how to see Welcome
// again.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../components"

QQC2.ScrollView {
    id: step

    property var welcome: ({})
    property var model: null

    contentWidth: availableWidth

    ColumnLayout {
        width: Math.min(step.availableWidth, Kirigami.Units.gridUnit * 40)
        spacing: Kirigami.Units.largeSpacing

        UqLabel {
            text: "Your desktop is ready. Everything you chose here, and much more, is in Settings: open quick settings with Super + A and press Settings, or find Settings in the app menu."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            text: "Keys to know"
            font.bold: true
        }
        KeybindList {
            objectName: "allKeys"
            binds: (step.welcome.keybinds || {}).all || []
            Layout.fillWidth: true
        }
        UqLabel {
            text: "To see Welcome again, open Settings, then About, and press Show Welcome."
            wrapMode: Text.Wrap
            color: Kirigami.Theme.disabledTextColor
            Layout.fillWidth: true
        }
    }
}
