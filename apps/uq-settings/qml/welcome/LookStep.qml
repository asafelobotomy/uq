// Welcome, step 2: the theme, light or dark, and switching between them on a schedule. It uses
// the Appearance page's model, so each choice applies to the desktop at once.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"

QQC2.ScrollView {
    id: step

    property var welcome: ({})
    property var model: null

    readonly property var snap: {
        try {
            return JSON.parse(am.stateJson);
        } catch (e) {
            return {};
        }
    }

    contentWidth: availableWidth

    AppearanceModel {
        id: am
    }
    Component.onCompleted: am.refresh()

    ColumnLayout {
        width: Math.min(step.availableWidth, Kirigami.Units.gridUnit * 40)
        spacing: Kirigami.Units.smallSpacing

        UqLabel {
            text: "Pick a theme and see it on the whole desktop straight away."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        ErrorCard {
            visible: am.error !== ""
            title: "That change wasn't applied"
            detail: am.error
            Layout.fillWidth: true
        }
        Flow {
            objectName: "themes"
            spacing: Kirigami.Units.smallSpacing
            enabled: App.uqdUp
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            Repeater {
                model: step.snap.themes || []
                QQC2.Button {
                    required property string modelData
                    readonly property bool current: modelData === step.snap.theme
                    checkable: true
                    checked: current
                    highlighted: current
                    Accessible.name: modelData
                    contentItem: UqLabel {
                        text: modelData
                        horizontalAlignment: Text.AlignHCenter
                        font.bold: current
                        padding: Kirigami.Units.smallSpacing
                    }
                    onClicked: {
                        am.setTheme(modelData, step.snap.variant || "dark");
                        checked = Qt.binding(() => current);
                    }
                }
            }
        }
        RowLayout {
            objectName: "variants"
            enabled: App.uqdUp
            Layout.topMargin: Kirigami.Units.largeSpacing
            Repeater {
                model: [{ id: "light", name: "Light" }, { id: "dark", name: "Dark" }]
                QQC2.Button {
                    required property var modelData
                    readonly property bool current: modelData.id === step.snap.variant
                    checkable: true
                    checked: current
                    highlighted: current
                    Accessible.name: modelData.name
                    contentItem: UqLabel {
                        text: modelData.name
                        horizontalAlignment: Text.AlignHCenter
                        font.bold: current
                        padding: Kirigami.Units.smallSpacing
                    }
                    onClicked: {
                        am.setTheme(step.snap.theme || "", modelData.id);
                        checked = Qt.binding(() => current);
                    }
                }
            }
        }
        ScheduleEditor {
            schedule: step.snap.schedule || { mode: "off" }
            enabled: App.uqdUp
            onApply: json => am.setSchedule(json)
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
    }
}
