import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 600
    height: 300

    Component {
        id: comp
        ProfileKeyList {
            width: 600
            editable: true
            keys: [
                { domain: "look", path: ["gaps_in"], label: "Gaps between windows", text: "10" },
                { domain: "power", path: ["profile"], label: "Power mode", text: "<b>Performance</b>" }
            ]
        }
    }

    TestCase {
        name: "ProfileKeyList"
        when: windowShown

        function row(list, i) {
            return findChild(list, "keyRow" + i);
        }

        function test_rows_show_labels_and_values() {
            const l = createTemporaryObject(comp, root);
            compare(findChild(row(l, 0), "keyLabel").text, "Gaps between windows");
            compare(findChild(row(l, 0), "keyText").text, "10");
            compare(findChild(row(l, 1), "keyLabel").text, "Power mode");
            // Values are shown as plain text, never as markup.
            compare(findChild(row(l, 1), "keyText").text, "<b>Performance</b>");
            compare(findChild(row(l, 1), "keyText").textFormat, Text.PlainText);
            verify(!findChild(l, "empty").visible);
        }

        function test_remove_emits_domain_and_path() {
            const l = createTemporaryObject(comp, root);
            const got = [];
            l.removeKey.connect((domain, pathJson) => got.push([domain, pathJson]));
            findChild(row(l, 0), "remove").clicked();
            compare(got, [["look", '["gaps_in"]']]);
        }

        function test_not_editable_hides_remove() {
            const l = createTemporaryObject(comp, root, { editable: false });
            verify(!findChild(row(l, 0), "remove").visible);
            verify(!findChild(row(l, 1), "remove").visible);
        }

        function test_empty_list_says_so() {
            const l = createTemporaryObject(comp, root, { keys: [] });
            const empty = findChild(l, "empty");
            verify(empty.visible);
            compare(empty.text, "No settings in this profile yet");
            compare(row(l, 0), null);
        }
    }
}
