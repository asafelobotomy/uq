import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "WorkspaceRules"
    visible: true
    when: windowShown
    width: 600
    height: 300

    Component {
        id: comp
        WorkspaceRules {
            width: 600
            layoutNames: ["dwindle", "master", "scrolling"]
            layoutLabels: ({ dwindle: "Dwindle", master: "Master", scrolling: "Scrolling" })
            workspaces: ({ "2": "scrolling" })
        }
    }

    function test_draft_survives_refresh_and_clears_after_the_add_lands() {
        const r = createTemporaryObject(comp, tc);
        const sent = [];
        r.setRule.connect((s, l) => sent.push([s, l]));
        r.selectorField.text = "5";
        r.workspaces = ({ "2": "scrolling", "7": "master" }); // a refresh from a keybind
        compare(r.selectorField.text, "5", "a half-typed selector is kept");
        r.pending = "5";
        r.workspaces = ({ "2": "scrolling", "7": "master" }); // refused add: still not saved
        compare(r.selectorField.text, "5");
        r.workspaces = ({ "2": "scrolling", "5": "dwindle", "7": "master" });
        compare(r.selectorField.text, "", "cleared once the rule is saved");
    }

    function test_unrelated_refresh_keeps_the_rows() {
        const r = createTemporaryObject(comp, tc);
        const key = r.ruleKey;
        r.workspaces = ({ "2": "master" });
        compare(r.ruleKey, key);
        compare(r.workspaces["2"], "master");
    }
}
