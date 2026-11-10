import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 600
    height: 300

    Component {
        id: comp
        ProfileMessage {
            width: 600
        }
    }

    TestCase {
        name: "ProfileMessage"
        when: windowShown

        function test_kept_label_with_markup_shows_literally() {
            const m = createTemporaryObject(comp, root, {
                kept: ["Workspace <b>x</b> layout", "Gaps between windows"],
                notices: ["Power mode wasn't changed: <a href='x'>y</a>."]
            });
            verify(m.visible);
            const first = findChild(m, "messageLine0");
            compare(first.text, "Kept your own value: Workspace <b>x</b> layout, Gaps between windows");
            compare(first.textFormat, Text.PlainText);
            const second = findChild(m, "messageLine1");
            compare(second.text, "Power mode wasn't changed: <a href='x'>y</a>.");
            compare(second.textFormat, Text.PlainText);
        }

        function test_empty_is_hidden() {
            const m = createTemporaryObject(comp, root);
            verify(!m.visible);
        }

        function test_close_hides_it() {
            const m = createTemporaryObject(comp, root, { notices: ["3 of those changes can't go in a profile"] });
            verify(m.visible);
            findChild(m, "close").clicked();
            verify(!m.visible);
        }
    }
}
