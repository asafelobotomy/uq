import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

// The keys Welcome lists, as plain text in two columns.
TestCase {
    id: tc
    name: "KeybindList"
    visible: true
    when: windowShown
    width: 400
    height: 200

    Component {
        id: comp
        KeybindList {}
    }

    function test_rows_are_plain_text() {
        const list = createTemporaryObject(comp, tc, {
            binds: [{ keys: "Super + Space", does: "Search" }, { keys: "<b>x</b>", does: "Tap" }]
        });
        verify(list);
        waitForRendering(list);
        const labels = [];
        for (let i = 0; i < list.children.length; i++) {
            const c = list.children[i];
            if (c.text !== undefined)
                labels.push(c);
        }
        compare(labels.length, 4);
        verify(labels.every(l => l.textFormat === Text.PlainText));
        verify(labels.some(l => l.text === "<b>x</b>"));
        verify(labels.some(l => l.text === "Super + Space" && l.font.bold));
    }

    function test_empty() {
        const list = createTemporaryObject(comp, tc);
        verify(list);
        compare(list.binds.length, 0);
    }
}
