// The per-workspace layout rules: a row for each rule (layout and Remove), and an Add row.
// `workspaces` is uqd's saved `{selector: layout}`. A half-typed Add draft is cleared only once
// the saved rules hold the selector that was sent, so a refused add, or a refresh from
// elsewhere, keeps what was typed.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

ColumnLayout {
    id: root

    property var workspaces: ({})
    property var layoutNames: []
    property var layoutLabels: ({})
    property string highlight
    /// The selector sent by Add, until the saved rules hold it.
    property string pending: ""
    /// Keyed by a string, so a refresh that leaves the rules alone doesn't rebuild their rows
    /// (which would close a combo the user has open).
    readonly property string ruleKey: JSON.stringify(Object.keys(workspaces || {}).sort())
    readonly property alias selectorField: selector
    /// Layout of `selector` ("" removes the rule).
    signal setRule(string selector, string layout)

    function labels() {
        return layoutNames.map(n => layoutLabels[n] || n);
    }

    onWorkspacesChanged: {
        if (pending !== "" && workspaces[pending] !== undefined) {
            pending = "";
            selector.text = "";
            newLayout.currentIndex = 0;
        }
    }
    spacing: Kirigami.Units.smallSpacing

    SettingRow {
        settingId: "workspace-layouts"
        label: "Layout for each workspace"
        highlight: root.highlight
        enabled: root.enabled
        Layout.fillWidth: true
        UqLabel {
            text: "No workspace has its own layout."
            visible: JSON.parse(root.ruleKey).length === 0
        }
    }
    Repeater {
        model: JSON.parse(root.ruleKey)
        SettingRow {
            id: rule
            required property string modelData
            readonly property string layout: root.workspaces[modelData] || ""
            objectName: "rule"
            label: "Workspace " + modelData
            enabled: root.enabled
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
            UqComboBox {
                model: root.labels()
                currentIndex: root.layoutNames.indexOf(rule.layout)
                Accessible.name: "Layout for workspace " + rule.modelData
                onActivated: index => {
                    root.setRule(rule.modelData, root.layoutNames[index]);
                    currentIndex = Qt.binding(() => root.layoutNames.indexOf(rule.layout));
                }
            }
            QQC2.Button {
                text: "Remove"
                Accessible.name: "Remove the rule for workspace " + rule.modelData
                onClicked: root.setRule(rule.modelData, "")
            }
        }
    }
    SettingRow {
        objectName: "addRule"
        label: "Add a workspace rule"
        enabled: root.enabled
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        QQC2.TextField {
            id: selector
            placeholderText: "Workspace, such as 3 or special:scratch"
            Accessible.name: "Workspace to add a rule for"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 14
        }
        UqComboBox {
            id: newLayout
            model: root.labels()
            Accessible.name: "Layout for the new rule"
        }
        QQC2.Button {
            objectName: "addButton"
            text: "Add"
            enabled: selector.text.trim() !== ""
            Accessible.name: "Add the workspace rule"
            onClicked: {
                root.pending = selector.text.trim();
                root.setRule(root.pending, root.layoutNames[newLayout.currentIndex]);
            }
        }
    }
}
