import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 700
    height: 500

    Component {
        id: comp
        AppRuleList {
            width: 700
        }
    }

    SignalSpy {
        id: turnedSpy
        signalName: "turned"
    }

    readonly property var romforge: ({
            id: "romforge.emulators",
            integration: "romforge",
            name: "<b>Emulators</b> open full screen",
            matches_text: ["App ID matches “^(retroarch)$”"],
            effects_text: ["Opens full screen", "Opens on workspace game"],
            enabled: false
        })

    TestCase {
        name: "AppRuleList"
        when: windowShown

        function make(apps, canChange) {
            const l = createTemporaryObject(comp, root, {
                apps: apps,
                canChange: canChange === undefined ? true : canChange
            });
            turnedSpy.target = l;
            turnedSpy.clear();
            return l;
        }

        function test_hidden_when_there_are_none() {
            verify(!make({}).visible);
            verify(!make({ rules: [], problems: [], missing: [] }).visible);
        }

        function test_rule_row_is_plain_text_with_a_switch() {
            const l = make({ rules: [root.romforge], problems: [], missing: [] });
            verify(l.visible);
            const lines = l.detailLines(root.romforge);
            compare(lines[0], "From romforge");
            compare(lines.length, 4);
            const line = findChild(l, "appRuleLine");
            verify(line);
            compare(line.textFormat, Text.PlainText);
            const sw = findChild(l, "appRuleSwitch");
            verify(sw);
            verify(!sw.checked);
            verify(sw.enabled);
        }

        function test_switch_asks_and_follows_the_list() {
            const l = make({ rules: [root.romforge], problems: [], missing: [] });
            const sw = findChild(l, "appRuleSwitch");
            mouseClick(sw);
            compare(turnedSpy.count, 1);
            compare(turnedSpy.signalArguments[0][0], "romforge.emulators");
            compare(turnedSpy.signalArguments[0][1], true);
            // Until uqd says it's on, the switch shows the list (refused: stays off).
            verify(!sw.checked);
        }

        function test_switch_off_while_busy() {
            const l = make({ rules: [root.romforge], problems: [], missing: [] }, false);
            verify(!findChild(l, "appRuleSwitch").enabled);
        }

        function test_problems_and_missing_show_alone() {
            const l = make({
                rules: [],
                problems: [{ integration: "broken", error: "rules.toml line 3: <i>bad</i>" }],
                missing: ["gone.rule"]
            });
            verify(l.visible);
            const p = findChild(l, "appRuleProblems");
            verify(p.visible);
            compare(p.shown[0], "The rules from broken can't be used: rules.toml line 3: <i>bad</i>");
            const m = findChild(l, "appRuleMissing");
            verify(m.visible);
            verify(m.shown[0].indexOf("gone.rule") >= 0);
            compare(findChild(l, "appRuleSwitch"), null);
        }
    }
}
