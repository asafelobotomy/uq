// "Advanced details": a closed-by-default disclosure with the raw Hyprland names of what a rule
// sets, for people who also write user.lua. This is the one file the plain-words lint exempts
// (scripts/lint-jargon.awk); every other file says it in plain words. All text is plain.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: details

    /// "window", "layer" or "workspace".
    property string kind: "window"
    /// The rule's id ("" for a rule not saved yet); window and panel rules are named `uq:<id>`.
    property string ruleId
    /// `[{label, lua}]` of the fields the rule sets, from the `fields` catalogue.
    property var rows: []
    readonly property alias expanded: toggle.checked
    readonly property string call: kind === "layer" ? "hl.layer_rule" : kind === "workspace" ? "hl.workspace_rule" : "hl.window_rule"
    readonly property string luaName: kind !== "workspace" && ruleId !== "" ? "uq:" + ruleId : ""

    spacing: Kirigami.Units.smallSpacing

    QQC2.ToolButton {
        id: toggle
        objectName: "advancedToggle"
        text: "Advanced details"
        icon.name: checked ? "arrow-down" : "arrow-right"
        checkable: true
        display: QQC2.AbstractButton.TextBesideIcon
    }
    ColumnLayout {
        visible: toggle.checked
        spacing: Kirigami.Units.smallSpacing
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing * 2

        UqLabel {
            objectName: "advancedCall"
            text: "Written to generated.lua as " + details.call + "."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "advancedName"
            visible: details.luaName !== ""
            text: "Rule name in Hyprland: " + details.luaName + ". In user.lua, " + details.call + "({ name = \"" + details.luaName + "\", enabled = false }) turns it off."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            visible: details.kind !== "workspace" && details.ruleId === ""
            text: "The Hyprland rule name is given when the rule is saved."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }
        Repeater {
            model: details.rows || []
            delegate: UqLabel {
                required property var modelData
                text: String(modelData.label) + ": " + String(modelData.lua)
                font.family: "monospace"
                Layout.fillWidth: true
            }
        }
    }
}
