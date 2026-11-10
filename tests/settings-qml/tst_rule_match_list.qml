import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 600
    height: 300

    Component {
        id: comp
        RuleMatchList {
            width: 600
        }
    }

    TestCase {
        name: "RuleMatchList"
        when: windowShown

        function make(matches) {
            return createTemporaryObject(comp, root, { matches: matches });
        }

        function test_rows_are_plain_text() {
            const l = make([{ title: "<b>x</b>", "class": "<i>app</i>", verdict: "yes" }]);
            const title = findChild(l, "matchTitle");
            verify(title);
            compare(title.text, "<b>x</b>");
            compare(title.textFormat, Text.PlainText);
            const app = findChild(l, "matchClass");
            compare(app.text, "<i>app</i>");
            compare(app.textFormat, Text.PlainText);
            verify(!findChild(l, "matchVerdict").visible, "a sure match has no note");
            verify(!findChild(l, "noMatches").visible);
        }

        function test_empty_says_so() {
            const l = make([]);
            const empty = findChild(l, "noMatches");
            verify(empty.visible);
            compare(empty.text, "No open window matches. You can still save the rule.");
            compare(findChild(l, "matchTitle"), null);
        }

        function test_if_dialog_row() {
            const l = make([{ title: "Save as", "class": "firefox", verdict: "if_dialog" }]);
            const note = findChild(l, "matchVerdict");
            verify(note.visible);
            compare(note.text, "Matches if it's a dialog");
        }

        function test_unknown_row() {
            const l = make([{ title: "Notes", "class": "kate", verdict: "unknown" }]);
            const note = findChild(l, "matchVerdict");
            verify(note.visible);
            compare(note.text, "Can't tell from here");
        }
    }
}
