import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 600
    height: 300

    Component {
        id: comp
        RuleMessage {
            width: 600
        }
    }

    TestCase {
        name: "RuleMessage"
        when: windowShown

        function test_reopen_lines() {
            const m = createTemporaryObject(comp, root);
            verify(!m.visible);
            compare(m.reopenText(["Firefox"], 0), "Close and reopen Firefox to apply.");
            compare(m.reopenText(["Firefox", "kitty"], 0), "Close and reopen Firefox and kitty to apply.");
            compare(m.reopenText(["Firefox", "kitty", "Volume Control"], 2),
                    "Close and reopen Firefox, kitty, Volume Control and 2 more to apply.");
            m.show([m.reopenText(["Firefox", "kitty", "Volume Control"], 0)]);
            verify(m.visible);
            compare(findChild(m, "messageLine0").text, "Close and reopen Firefox, kitty and Volume Control to apply.");
            findChild(m, "close").clicked();
            verify(!m.visible);
            m.show(["again"]);
            verify(m.visible, "a new message shows again");
        }

        function test_rich_text_not_rendered() {
            const m = createTemporaryObject(comp, root, {
                lines: ["Close and reopen <b>x</b> to apply.", "<a href='y'>z</a>"]
            });
            const first = findChild(m, "messageLine0");
            compare(first.text, "Close and reopen <b>x</b> to apply.");
            compare(first.textFormat, Text.PlainText);
            const second = findChild(m, "messageLine1");
            compare(second.text, "<a href='y'>z</a>");
            compare(second.textFormat, Text.PlainText);
        }

        function test_pick_statuses() {
            const m = createTemporaryObject(comp, root);
            compare(m.pickText("none"), "No window there.");
            compare(m.pickText("cancelled"), "Choosing a window was cancelled.");
            compare(m.pickText("no_shell"), "The desktop shell didn't answer. Try again.");
            compare(m.pickText("timeout"), "Took too long; nothing was chosen.");
            compare(m.pickText("unavailable"), "Couldn't start choosing a window.");
            compare(m.pickText("error"), "Couldn't start choosing a window.");
            compare(m.pickText("picked"), "");
        }
    }
}
