// How the workspaces spread over the screens: off, one after another, in turn, or chosen for
// each. The preview shows the result for the screens connected now, one row for each screen.
// `plan` is the snapshot's `{strategy, count, move_back, manual, screens, preview}`. Screen
// names come from the screens, so they are shown through UqLabel.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

ColumnLayout {
    id: root

    property var plan: ({ strategy: "off", count: 10, move_back: true, manual: [], screens: [], preview: [] })
    property string highlight
    readonly property alias strategyBox: strategyCombo
    readonly property alias countBox: countSpin
    readonly property alias backSwitch: backRow
    readonly property alias previewRepeater: previewRows
    readonly property alias manualRepeater: manualRows
    readonly property var keys: ["off", "sequential", "interleave", "manual"]
    readonly property string strategy: plan && plan.strategy ? plan.strategy : "off"
    readonly property int count: plan && plan.count ? plan.count : 10
    readonly property bool moveBack: !plan || plan.move_back !== false
    readonly property var screens: plan && plan.screens ? plan.screens : []
    readonly property var preview: plan && plan.preview ? plan.preview : []
    readonly property var manual: plan && plan.manual ? plan.manual : []

    /// The planner was changed: how to spread, how many workspaces, and whether to move them
    /// back when a screen returns.
    signal planChosen(string strategy, int count, bool moveBack)
    /// Workspace `workspace` was put on the display `output` ("" = no choice).
    signal manualChosen(int workspace, string output)

    /// "1 to 5", "3" or "1, 3, 5".
    function describe(ws) {
        if (ws.length === 0)
            return "";
        if (ws.length === 1)
            return String(ws[0]);
        const run = ws[ws.length - 1] - ws[0] === ws.length - 1;
        return run ? ws[0] + " to " + ws[ws.length - 1] : ws.join(", ");
    }
    function slotIndex(workspace) {
        const slot = manual.find(s => s.workspace === workspace);
        if (!slot || !slot.output)
            return 0;
        return screens.findIndex(s => s.output === slot.output) + 1;
    }

    spacing: Kirigami.Units.smallSpacing

    SettingRow {
        settingId: "display-workspaces"
        iconName: "virtual-desktops"
        label: "Spread workspaces"
        help: "Off: workspaces aren't tied to a screen. One after another: the first screen gets the first ones. In turn: they alternate between screens."
        highlight: root.highlight
        enabled: root.enabled
        Layout.fillWidth: true

        UqComboBox {
            id: strategyCombo
            objectName: "strategyBox"
            model: ["Off", "One after another", "In turn", "Choose for each"]
            currentIndex: Math.max(0, root.keys.indexOf(root.strategy))
            Accessible.name: "Workspaces across screens"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 16
            onActivated: index => {
                root.planChosen(root.keys[index], root.count, root.moveBack);
                currentIndex = Qt.binding(() => Math.max(0, root.keys.indexOf(root.strategy)));
            }
        }
    }

    SettingRow {
        objectName: "countRow"
        visible: root.strategy !== "off"
        iconName: "view-list-details"
        label: "Number of workspaces"
        enabled: root.enabled
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing * 2

        QQC2.SpinBox {
            id: countSpin
            objectName: "countBox"
            from: 1
            to: 20
            value: root.count
            editable: true
            Accessible.name: "Number of workspaces"
            onValueModified: {
                root.planChosen(root.strategy, value, root.moveBack);
                value = Qt.binding(() => root.count);
            }
        }
    }
    SwitchRow {
        id: backRow
        settingId: "display-workspaces-back"
        objectName: "moveBackRow"
        visible: root.strategy !== "off"
        iconName: "edit-undo"
        label: "Move workspaces back when a screen returns"
        highlight: root.highlight
        enabled: root.enabled
        value: root.moveBack
        onChosen: v => root.planChosen(root.strategy, root.count, v)
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing * 2
    }

    // Each screen with its workspaces as numbered tiles: the plan at a glance.
    ColumnLayout {
        objectName: "preview"
        visible: root.preview.length > 0
        spacing: Kirigami.Units.smallSpacing
        Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        Layout.rightMargin: Kirigami.Units.largeSpacing
        Layout.topMargin: Kirigami.Units.smallSpacing
        Layout.fillWidth: true

        Repeater {
            id: previewRows
            model: root.preview
            RowLayout {
                id: row
                required property var modelData
                readonly property string screenLabel: String(modelData.label)
                readonly property string workspacesText: "Workspaces " + root.describe(modelData.workspaces)
                objectName: "previewRow"
                spacing: Kirigami.Units.largeSpacing
                Layout.fillWidth: true
                Accessible.role: Accessible.StaticText
                Accessible.name: row.screenLabel + ": " + row.workspacesText

                Kirigami.Icon {
                    source: "video-display"
                    implicitWidth: Kirigami.Units.iconSizes.smallMedium
                    implicitHeight: Kirigami.Units.iconSizes.smallMedium
                    Accessible.ignored: true
                }
                UqLabel {
                    objectName: "previewScreen"
                    text: row.screenLabel
                    elide: Text.ElideRight
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 10
                }
                Flow {
                    objectName: "previewWorkspaces"
                    spacing: Kirigami.Units.smallSpacing
                    Layout.fillWidth: true

                    Repeater {
                        model: row.modelData.workspaces || []
                        Rectangle {
                            required property var modelData
                            width: Kirigami.Units.gridUnit * 1.8
                            height: width
                            radius: Kirigami.Units.cornerRadius
                            color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.2)
                            border.width: 1
                            border.color: Kirigami.Theme.highlightColor
                            UqLabel {
                                anchors.centerIn: parent
                                text: String(parent.modelData)
                                font.bold: true
                            }
                        }
                    }
                }
            }
        }
    }

    Repeater {
        id: manualRows
        model: root.strategy === "manual" ? root.count : 0
        SettingRow {
            id: slot
            required property int index
            readonly property int workspace: index + 1
            objectName: "manualRow"
            label: "Workspace " + workspace
            enabled: root.enabled
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2

            UqComboBox {
                objectName: "manualBox"
                model: ["Not chosen"].concat(root.screens.map(s => s.label))
                currentIndex: root.slotIndex(slot.workspace)
                // A new model (a screen came or went) resets the index.
                onModelChanged: Qt.callLater(() => currentIndex = Qt.binding(() => root.slotIndex(slot.workspace)))
                Accessible.name: "Display for workspace " + slot.workspace
                Layout.preferredWidth: Kirigami.Units.gridUnit * 16
                onActivated: index => {
                    root.manualChosen(slot.workspace, index === 0 ? "" : root.screens[index - 1].output);
                    currentIndex = Qt.binding(() => root.slotIndex(slot.workspace));
                }
            }
        }
    }
}
