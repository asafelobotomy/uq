// "Advanced details": a closed-by-default disclosure with the raw Hyprland names of what a rule
// sets, for people who also write user.lua, (kind "fix") the variable names and options an app
// fix adds to the app's command, or (kind "shortcuts") the id each keyboard shortcut goes by. This is the one file the plain-words lint exempts
// (scripts/lint-jargon.awk); every other file says it in plain words. All text is plain.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: details

    /// "window", "layer", "workspace", "fix" or "shortcuts".
    property string kind: "window"
    /// The rule's id ("" for a rule not saved yet); window and panel rules are named `uq:<id>`.
    property string ruleId
    /// `[{label, lua}]` of the fields the rule sets, from the `fields` catalogue; for "shortcuts",
    /// of every shortcut and the id it goes by.
    property var rows: []
    /// For a fix: the variable names it sets, the options it adds, and why it can't be used
    /// ("" when it can), from CompatModel.preview.
    property var envNames: []
    property var extraArgs: []
    property string problem
    readonly property bool isRule: kind !== "fix" && kind !== "shortcuts"
    readonly property bool isShortcuts: kind === "shortcuts"
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
            visible: details.isRule
            text: "Written to generated.lua as " + details.call + "."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "advancedName"
            visible: details.isRule && details.luaName !== ""
            text: "Rule name in Hyprland: " + details.luaName + ". In user.lua, " + details.call + "({ name = \"" + details.luaName + "\", enabled = false }) turns it off."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            visible: details.isRule && details.kind !== "workspace" && details.ruleId === ""
            text: "The Hyprland rule name is given when the rule is saved."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "advancedFixEnv"
            visible: details.kind === "fix" && details.problem === ""
            text: (details.envNames || []).length > 0 ? "Environment variables set for this app: " + details.envNames.map(n => String(n)).join(", ") : "No environment variables are set for this app."
            font.family: "monospace"
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "advancedFixArgs"
            visible: details.kind === "fix" && details.problem === "" && (details.extraArgs || []).length > 0
            text: "Added to its command line: " + (details.extraArgs || []).map(a => String(a)).join(" ")
            font.family: "monospace"
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "advancedFixProblem"
            visible: details.kind === "fix" && details.problem !== ""
            text: details.problem
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "advancedShortcuts"
            visible: details.isShortcuts
            text: "Each shortcut is written to generated.lua as a bind described as uq:<id>. In user.lua, hl.unbind(\"SUPER + Space\") turns the one on those keys off."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        Repeater {
            model: details.isShortcuts ? (details.rows || []) : []
            delegate: UqLabel {
                required property var modelData
                text: String(modelData.label) + ": " + String(modelData.lua)
                font.family: "monospace"
                Layout.fillWidth: true
            }
        }
        Repeater {
            model: details.isRule ? (details.rows || []) : []
            delegate: UqLabel {
                required property var modelData
                text: String(modelData.label) + ": " + String(modelData.lua)
                font.family: "monospace"
                Layout.fillWidth: true
            }
        }
    }
}
