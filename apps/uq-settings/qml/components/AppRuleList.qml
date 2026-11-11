// "Rules from apps" on the Window rules page (Phase 4 spec §3.3): window rules installed apps
// offer in their own files, each off until turned on here. One row per rule: its name, which app
// it comes from, what it matches and does, and a switch. Files that couldn't be used, and rules
// that are on but no longer installed, are said under the ruleList. Hidden when there is nothing.
// Every name, pattern and reason comes from an app's file, so all text is UqLabel (plain text).
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: ruleList

    /// `{rules, problems, missing}` as RulesModel gives it under `apps`.
    property var apps: ({})
    /// Whether a switch may be used now (uqd is up and nothing is in flight).
    property bool canChange: true
    /// For search: the highlighted setting's id.
    property string highlight
    readonly property var rules: ruleList.apps && ruleList.apps.rules ? ruleList.apps.rules : []
    readonly property var problems: ruleList.apps && ruleList.apps.problems ? ruleList.apps.problems : []
    readonly property var missing: ruleList.apps && ruleList.apps.missing ? ruleList.apps.missing : []

    /// The person moved a switch: turn rule `id` on or off. The switch goes back to what the
    /// list says until the list comes back changed.
    signal turned(string id, bool on)

    /// The lines under a rule's name.
    function detailLines(rule) {
        const out = ["From " + String(rule.integration)];
        for (const l of rule.matches_text || [])
            out.push(String(l));
        for (const l of rule.effects_text || [])
            out.push(String(l));
        return out;
    }

    visible: ruleList.rules.length + ruleList.problems.length + ruleList.missing.length > 0
    spacing: Kirigami.Units.smallSpacing
    Accessible.role: Accessible.Grouping
    Accessible.name: "Rules from apps"

    UqLabel {
        text: "Rules from apps"
        font.bold: true
        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.2
        Accessible.role: Accessible.Heading
    }
    SettingRow {
        settingId: "rules-from-apps"
        label: "Apps you installed can offer rules of their own. Each one is off until you turn it on."
        highlight: ruleList.highlight
        Layout.fillWidth: true
    }
    Repeater {
        model: ruleList.rules
        delegate: ColumnLayout {
            id: row

            required property var modelData
            readonly property string title: String(row.modelData.name)

            objectName: "appRule"
            spacing: 0
            Layout.fillWidth: true
            Accessible.role: Accessible.Grouping
            Accessible.name: row.title

            SettingRow {
                label: row.title
                Layout.fillWidth: true

                QQC2.Switch {
                    id: onSwitch
                    objectName: "appRuleSwitch"
                    checked: row.modelData.enabled === true
                    enabled: ruleList.canChange
                    Accessible.name: "Rule on"
                    Accessible.description: row.title
                    onToggled: {
                        ruleList.turned(String(row.modelData.id), onSwitch.checked);
                        onSwitch.checked = Qt.binding(() => row.modelData.enabled === true);
                    }
                }
            }
            Repeater {
                model: ruleList.detailLines(row.modelData)
                delegate: UqLabel {
                    required property string modelData
                    objectName: "appRuleLine"
                    text: modelData
                    wrapMode: Text.Wrap
                    opacity: 0.7
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing * 2
                }
            }
        }
    }
    RuleMessage {
        objectName: "appRuleProblems"
        negative: true
        closable: false
        lines: ruleList.problems.map(p => "The rules from " + String(p.integration) + " can't be used: " + String(p.error))
        Layout.fillWidth: true
    }
    RuleMessage {
        objectName: "appRuleMissing"
        closable: false
        lines: ruleList.missing.map(id => "Turned on, but its app isn't installed now: " + String(id) + ". It applies again when the app is back.")
        Layout.fillWidth: true
    }
}
