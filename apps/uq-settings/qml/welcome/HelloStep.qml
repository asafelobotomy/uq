// Welcome, step 1: what uQ is, and the three keys to know. When uq-session's first-login setup
// reported a problem, a card says so and names the log.
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

        ErrorCard {
            objectName: "setupProblem"
            visible: step.welcome.setup_problem === true
            title: "Setup reported a problem"
            detail: "Something went wrong while setting up uQ the first time. The details are in " + (step.welcome.session_log || "") + ". Your desktop works, but some files may be missing."
            Layout.fillWidth: true
        }
        UqLabel {
            text: "uQ is your desktop: the bar at the top, the launcher, quick settings, notifications and the lock screen, all in one look. The next few steps set it up the way you like. You can skip any of them and change everything later in Settings."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            text: "Three keys to know"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        KeybindList {
            objectName: "firstKeys"
            binds: (step.welcome.keybinds || {}).first || []
            Layout.fillWidth: true
        }
        UqLabel {
            text: "Super is the key with the Windows or command logo."
            wrapMode: Text.Wrap
            color: Kirigami.Theme.disabledTextColor
            Layout.fillWidth: true
        }
    }
}
