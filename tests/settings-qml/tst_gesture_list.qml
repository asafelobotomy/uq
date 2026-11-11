import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 700
    height: 600

    readonly property var rowsData: [
        { label: "3 fingers, swipe sideways", action_label: "Switch workspace", enabled: true, gesture: { fingers: 3, direction: "horizontal", action: "workspace", enabled: true } },
        { label: "4 fingers, swipe up", action_label: "Open Spotlight", enabled: true, gesture: { fingers: 4, direction: "up", action: "uq:spotlight", enabled: true } }
    ]

    Component {
        id: comp
        GestureList {
            width: 600
            gestures: root.rowsData
            opensFiles: false
        }
    }

    Component {
        id: spyComp
        SignalSpy {}
    }

    TestCase {
        name: "GestureList"
        when: windowShown

        function make(props) {
            const c = createTemporaryObject(comp, root, props || {});
            verify(c);
            return c;
        }
        function spy(c, name) {
            const s = createTemporaryObject(spyComp, root, { target: c, signalName: name });
            verify(s.valid, name);
            return s;
        }

        function test_defaults_badge() {
            const c = make({ defaults: true });
            verify(c.badge.visible);
            compare(c.badge.text, "These are uQ's own gestures. Change one and the list becomes yours.");
            verify(!c.resetButton.visible, "nothing to go back to");
            c.defaults = false;
            verify(!c.badge.visible);
            verify(c.resetButton.visible);
            compare(c.resetButton.text, "Back to uQ's gestures");
            const got = spy(c, "resetRequested");
            c.resetButton.clicked();
            compare(got.count, 1);
        }

        function test_rows_show_what_you_do_and_what_it_does() {
            const c = make();
            compare(c.rows.count, 2);
            compare(c.rows.itemAt(0).nameLabel.text, "3 fingers, swipe sideways");
            compare(findChild(c.rows.itemAt(1), "gestureAction").text, "Open Spotlight");
            verify(c.rows.itemAt(0).toggle.checked);
            verify(!findChild(c, "noGestures").visible);
            verify(!c.rows.itemAt(0).clashLine.visible);
        }

        function test_toggle_and_remove() {
            const c = make({ defaults: false });
            const toggled = spy(c, "toggled");
            const removed = spy(c, "removed");
            const change = spy(c, "changeRequested");
            const add = spy(c, "addRequested");
            c.rows.itemAt(0).toggle.click();
            compare(toggled.count, 1);
            compare(toggled.signalArguments[0][0], 0);
            compare(toggled.signalArguments[0][1], false);
            verify(c.rows.itemAt(0).toggle.checked, "the switch waits for the list to change");
            c.rows.itemAt(1).removeButton.clicked();
            compare(removed.count, 1);
            compare(removed.signalArguments[0][0], 1);
            c.rows.itemAt(1).changeButton.clicked();
            compare(change.count, 1);
            compare(change.signalArguments[0][0], 1);
            c.addButton.clicked();
            compare(add.count, 1);
        }

        function test_off_gesture_shows_off() {
            const off = [{ label: "3 fingers, swipe left", action_label: "Close the window", enabled: false }];
            const c = make({ gestures: off });
            verify(!c.rows.itemAt(0).toggle.checked);
        }

        function test_conflict_line_under_its_row() {
            const c = make({ clashes: { "1": "“Open Spotlight” and “Switch workspace” both use 4 fingers, swipe up." } });
            verify(!c.rows.itemAt(0).clashLine.visible);
            verify(c.rows.itemAt(1).clashLine.visible);
            compare(c.rows.itemAt(1).clashLine.text, "“Open Spotlight” and “Switch workspace” both use 4 fingers, swipe up.");
            compare(c.rows.itemAt(1).clashLine.textFormat, Text.PlainText);
        }

        function test_user_lua_line_plain_text() {
            const c = make({ userLua: { line: 12, path: "/home/me/.config/uq/user.lua" } });
            verify(c.luaLine.visible);
            const text = findChild(c, "userLuaText");
            compare(text.text, "Overridden in user.lua, line 12");
            compare(text.textFormat, Text.PlainText);
            c.userLua = null;
            verify(!c.luaLine.visible);
        }

        function test_labels_are_plain_text() {
            const c = make({ gestures: [{ label: "<b>x</b>", action_label: "<i>y</i>", enabled: true }] });
            compare(c.rows.itemAt(0).nameLabel.text, "<b>x</b>");
            compare(c.rows.itemAt(0).nameLabel.textFormat, Text.PlainText);
            compare(findChild(c.rows.itemAt(0), "gestureAction").textFormat, Text.PlainText);
        }

        function test_empty_list_says_so() {
            const c = make({ gestures: [], defaults: false });
            verify(findChild(c, "noGestures").visible);
            compare(c.rows.count, 0);
        }

        function test_busy_disables_the_controls() {
            const c = make({ idle: false });
            verify(!c.addButton.enabled);
            verify(!c.rows.itemAt(0).toggle.enabled);
            verify(!c.rows.itemAt(0).removeButton.enabled);
        }

        function test_keyboard_only() {
            const c = make({ defaults: false });
            const controls = [c.addButton, c.resetButton, c.rows.itemAt(0).toggle, c.rows.itemAt(0).changeButton, c.rows.itemAt(0).removeButton, c.rows.itemAt(1).toggle, c.rows.itemAt(1).changeButton, c.rows.itemAt(1).removeButton];
            for (const ctl of controls)
                verify((ctl.focusPolicy & Qt.TabFocus) !== 0, ctl.objectName);
            // Space on Add Opens the editor.
            const add = spy(c, "addRequested");
            c.addButton.forceActiveFocus();
            tryVerify(() => c.addButton.activeFocus);
            keyClick(Qt.Key_Space);
            compare(add.count, 1);
        }
    }
}
