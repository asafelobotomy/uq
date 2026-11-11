// Welcome, step 4: how windows are arranged on a workspace by default. It uses the Look and
// layouts page's model, so the choice applies at once; that page also sets a layout per
// workspace and each layout's options.
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
            return JSON.parse(lm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property string chosen: (snap.layouts || {}).default || ((snap.defaults || {}).layouts || {}).default || "dwindle"
    /// Each layout with a plain description and a small picture of three windows in it
    /// (x, y, w, h as parts of the screen).
    readonly property var layouts: [
        { id: "dwindle", name: "Dwindle", text: "Each new window splits the space of the one you're in, so they spiral inwards.", boxes: [[0, 0, 0.5, 1], [0.5, 0, 0.5, 0.5], [0.5, 0.5, 0.5, 0.5]] },
        { id: "master", name: "Master", text: "One large window on the left, and the rest stacked beside it.", boxes: [[0, 0, 0.6, 1], [0.6, 0, 0.4, 0.5], [0.6, 0.5, 0.4, 0.5]] },
        { id: "scrolling", name: "Scrolling", text: "Windows sit side by side in columns, and the row scrolls sideways as you add more.", boxes: [[0, 0, 0.45, 1], [0.5, 0, 0.45, 1], [0.95, 0, 0.45, 1]] }
    ]

    contentWidth: availableWidth

    LookModel {
        id: lm
    }
    Component.onCompleted: lm.refresh()

    ColumnLayout {
        width: Math.min(step.availableWidth, Kirigami.Units.gridUnit * 40)
        spacing: Kirigami.Units.largeSpacing

        UqLabel {
            text: "uQ places windows for you. Pick how; you can still move and resize any window, and give a workspace its own layout later in Settings."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        ErrorCard {
            visible: lm.error !== ""
            title: "That change wasn't applied"
            detail: lm.error
            Layout.fillWidth: true
        }
        Repeater {
            model: step.layouts
            QQC2.ItemDelegate {
                id: choice
                required property var modelData
                readonly property bool current: modelData.id === step.chosen
                objectName: "layout-" + modelData.id
                enabled: App.uqdUp
                highlighted: current
                Accessible.role: Accessible.RadioButton
                Accessible.name: modelData.name
                Accessible.description: modelData.text
                Accessible.checked: current
                Layout.fillWidth: true
                onClicked: lm.setLayoutOption("", "default", JSON.stringify(modelData.id))
                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing
                    // A small picture of three windows in this layout.
                    Rectangle {
                        implicitWidth: Kirigami.Units.gridUnit * 5
                        implicitHeight: Kirigami.Units.gridUnit * 3
                        color: Kirigami.Theme.alternateBackgroundColor
                        border.color: choice.current ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
                        radius: 3
                        clip: true
                        Repeater {
                            model: choice.modelData.boxes
                            Rectangle {
                                required property var modelData
                                x: 2 + modelData[0] * (parent.width - 4)
                                y: 2 + modelData[1] * (parent.height - 4)
                                width: modelData[2] * (parent.width - 4) - 2
                                height: modelData[3] * (parent.height - 4) - 2
                                radius: 2
                                color: Kirigami.Theme.highlightColor
                                opacity: 0.55
                            }
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        UqLabel {
                            text: choice.modelData.name + (choice.current ? " (in use)" : "")
                            font.bold: true
                        }
                        UqLabel {
                            text: choice.modelData.text
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }
                    }
                }
            }
        }
    }
}
