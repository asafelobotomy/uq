import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "OptionGroup"
    visible: true
    when: windowShown
    width: 500
    height: 300

    Component {
        id: comp
        OptionGroup {
            width: 500
            settingId: "master-options"
            label: "Master layout options"
            UqLabel {
                text: "option"
            }
        }
    }

    function test_hidden_until_used() {
        const g = createTemporaryObject(comp, tc, { inUse: false });
        compare(g.visible, false);
        g.inUse = true;
        compare(g.visible, true);
        compare(findChild(g, "unusedNote").visible, false);
    }

    function test_search_hit_shows_an_unused_group_with_a_note() {
        const g = createTemporaryObject(comp, tc, { inUse: false });
        g.highlight = "scrolling-options";
        compare(g.visible, false, "another group's hit does not show it");
        g.highlight = "master-options";
        compare(g.visible, true);
        compare(findChild(g, "unusedNote").visible, true);
        g.highlight = "";
        compare(g.visible, false);
    }
}
