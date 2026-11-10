import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 600
    height: 300

    Component {
        id: comp
        KeyboardLayoutList {
            width: 600
            layouts: [{ layout: "us" }, { layout: "de", variant: "nodeadkeys" }]
            xkbLayouts: [["us", "English (US)"], ["de", "German"]]
            variants: ({ de: [["nodeadkeys", "German (no dead keys)"]] })
        }
    }

    TestCase {
        name: "KeyboardLayoutList"
        when: windowShown

        function button(list, row, name) {
            return findChild(findChild(list, "layoutRow" + row), name);
        }

        function test_move_up_reorders() {
            const l = createTemporaryObject(comp, root);
            const got = [];
            l.reordered.connect(v => got.push(v));
            button(l, 1, "up").clicked();
            compare(got.length, 1);
            compare(got[0][0].layout, "de");
            compare(got[0][1].layout, "us");
        }

        function test_first_up_and_last_down_disabled() {
            const l = createTemporaryObject(comp, root);
            verify(!button(l, 0, "up").enabled);
            verify(button(l, 0, "down").enabled);
            verify(!button(l, 1, "down").enabled);
        }

        function test_remove_emits_index() {
            const l = createTemporaryObject(comp, root);
            const got = [];
            l.removed.connect(i => got.push(i));
            button(l, 1, "remove").clicked();
            compare(got, [1]);
        }

        function test_names_and_default() {
            const l = createTemporaryObject(comp, root);
            compare(l.nameOf({ layout: "de", variant: "nodeadkeys" }), "German – German (no dead keys)");
            compare(l.nameOf({ layout: "xx" }), "xx");
        }

        function test_add_needs_a_layout() {
            const l = createTemporaryObject(comp, root);
            verify(!l.addButton.enabled);
            const got = [];
            l.added.connect((a, v) => got.push([a, v]));
            l.layoutPicker.currentIndex = 2;
            verify(l.addButton.enabled);
            l.addButton.clicked();
            compare(got, [["de", ""]]);
        }

        function test_code_field_without_system_list() {
            const l = createTemporaryObject(comp, root, { xkbLayouts: [] });
            verify(l.codeField.visible);
            verify(!l.addButton.enabled);
            const got = [];
            l.added.connect((a, v) => got.push([a, v]));
            l.codeField.text = " fr ";
            l.addButton.clicked();
            compare(got, [["fr", ""]]);
        }
    }
}
