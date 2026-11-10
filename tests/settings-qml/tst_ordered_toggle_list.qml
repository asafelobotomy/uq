import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "OrderedToggleList"
    visible: true
    when: windowShown
    width: 500
    height: 300

    Component {
        id: comp
        OrderedToggleList {
            width: 500
            model: [
                { id: "a", name: "Alpha", enabled: true },
                { id: "b", name: "<b>Beta</b>", enabled: true },
                { id: "c", name: "Gamma", enabled: false, missing: true }
            ]
        }
    }

    function test_moving_up_emits_new_order() {
        const l = createTemporaryObject(comp, tc);
        const got = [];
        l.reordered.connect(ids => got.push(ids));
        l.repeater.itemAt(1).upButton.clicked();
        compare(got, [["b", "a", "c"]]);
    }

    function test_moving_down_emits_new_order() {
        const l = createTemporaryObject(comp, tc);
        const got = [];
        l.reordered.connect(ids => got.push(ids));
        l.repeater.itemAt(0).downButton.clicked();
        compare(got, [["b", "a", "c"]]);
    }

    function test_toggle_emits_and_snaps_back() {
        const l = createTemporaryObject(comp, tc);
        const got = [];
        l.toggled.connect((id, on) => got.push([id, on]));
        const sw = l.repeater.itemAt(1).toggle;
        sw.toggle();
        sw.toggled();
        compare(got, [["b", false]]);
        compare(sw.checked, true, "shows the saved value until the model confirms");
    }

    function test_ends_have_no_move_past_them() {
        const l = createTemporaryObject(comp, tc);
        compare(l.repeater.itemAt(0).upButton.enabled, false);
        compare(l.repeater.itemAt(1).upButton.enabled, true);
        compare(l.repeater.itemAt(2).downButton.enabled, false);
    }

    function test_remove_and_switch_hidden() {
        const l = createTemporaryObject(comp, tc, { showSwitch: false, removable: true });
        compare(l.repeater.itemAt(0).toggle.visible, false);
        compare(l.repeater.itemAt(0).removeButton.visible, true);
        const got = [];
        l.removed.connect(id => got.push(id));
        l.repeater.itemAt(2).removeButton.clicked();
        compare(got, ["c"]);
    }
}
