import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 700
    height: 600

    Component {
        id: comp
        DisplayArrangements {
            width: 600
            arrangements: [{ id: "m-1", name: "Desk", screens: ["BOE 0x0BCA", "Dell U2723QE"], current: true }, { id: "m-2", name: "Laptop only", screens: ["BOE 0x0BCA"], current: false }]
        }
    }

    Component {
        id: spyComp
        SignalSpy {}
    }

    TestCase {
        name: "DisplayArrangements"
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

        function test_remember_banner_shown_for_offer() {
            const c = make({ offer: "" });
            verify(!c.offerBanner.visible);
            c.offer = "remember";
            verify(c.offerBanner.visible);
            compare(c.offerButtonText, "Remember for these screens");
            c.offer = "update";
            c.offerName = "Desk";
            verify(c.offerBanner.visible);
            compare(c.offerButtonText, "Update “Desk”");
        }

        function test_update_sends_the_name_without_asking() {
            const c = make({ offer: "update", offerName: "Desk" });
            const got = spy(c, "remember");
            c.offerButton.clicked();
            compare(got.count, 1);
            compare(got.signalArguments[0][0], "Desk");
        }

        function test_remember_asks_for_a_name() {
            const c = make({ offer: "remember" });
            const got = spy(c, "remember");
            c.offerButton.clicked();
            compare(got.count, 0);
            tryCompare(c, "naming", true);
            compare(c.nameDialog.title, "Remember these screens");
            c.nameField.text = "  Home  ";
            c.saveName();
            compare(got.count, 1);
            compare(got.signalArguments[0][0], "Home");
        }

        function test_rename_uses_the_same_dialog() {
            const c = make();
            const got = spy(c, "rename");
            c.rows.itemAt(1).renameButton.clicked();
            tryCompare(c, "naming", true);
            compare(c.nameDialog.title, "Rename arrangement");
            compare(c.nameField.text, "Laptop only");
            c.nameField.text = "Travel";
            c.saveName();
            compare(got.count, 1);
            compare(got.signalArguments[0][0], "m-2");
            compare(got.signalArguments[0][1], "Travel");
        }

        function test_empty_name_is_not_sent() {
            const c = make();
            const got = spy(c, "remember");
            c.startRemember();
            c.nameField.text = "   ";
            c.saveName();
            compare(got.count, 0);
        }

        function test_name_shown_as_plain_text() {
            const c = make({ arrangements: [{ id: "m-1", name: "<b>x</b>", screens: ["<i>y</i>"], current: false }], failed: true });
            const label = c.rows.itemAt(0).nameLabel;
            compare(label.text, "<b>x</b>");
            compare(label.textFormat, Text.PlainText);
            const sc = findChild(c.rows.itemAt(0), "arrangementScreens");
            compare(sc.textFormat, Text.PlainText);
            compare(sc.text, "<i>y</i>");
        }

        function test_forget_asks_first() {
            const c = make();
            const got = spy(c, "forget");
            c.rows.itemAt(0).forgetButton.clicked();
            compare(got.count, 0, "nothing is forgotten before the answer");
            compare(c.forgetId, "m-1");
            compare(c.forgetLabel.text, "Forget “Desk”? Your screens stay as they are.");
            c.confirmForgetNow();
            compare(got.count, 1);
            compare(got.signalArguments[0][0], "m-1");
            compare(c.forgetId, "");
        }

        function test_forget_cancelled_sends_nothing() {
            const c = make();
            const got = spy(c, "forget");
            c.askForget("m-2", "Laptop only");
            compare(c.forgetId, "m-2");
            // Cancel closes the dialog, which clears the pending forget.
            c.forgetId = "";
            c.confirmForgetNow();
            compare(got.count, 0);
        }

        function test_failed_line_shown() {
            const c = make({ failed: false });
            verify(!c.failedLabel.visible);
            c.failed = true;
            verify(c.failedLabel.visible);
            compare(c.failedLabel.text, "“Desk” couldn't be used with these screens");
            compare(c.failedLabel.textFormat, Text.PlainText);
        }

        function test_empty_list_says_so() {
            const c = make({ arrangements: [] });
            verify(findChild(c, "noArrangements").visible);
            compare(c.rows.count, 0);
        }

        function test_auto_switch_is_sent() {
            const c = make({ autoSwitch: true });
            const got = spy(c, "autoSwitchChosen");
            c.autoRow.toggle.click();
            compare(got.count, 1);
            compare(got.signalArguments[0][0], false);
        }

        function test_keyboard_only() {
            const c = make({ offer: "remember" });
            // Every control can take focus from the Tab key.
            const controls = [c.offerButton, c.rememberButton, c.rows.itemAt(0).renameButton, c.rows.itemAt(0).forgetButton, c.rows.itemAt(1).renameButton, c.rows.itemAt(1).forgetButton, c.autoRow.toggle];
            for (const ctl of controls)
                verify((ctl.focusPolicy & Qt.TabFocus) !== 0, ctl.objectName);
            // Enter on Remember opens the dialog.
            c.rememberButton.forceActiveFocus();
            tryVerify(() => c.rememberButton.activeFocus);
            keyClick(Qt.Key_Return);
            tryCompare(c, "naming", true);
        }
    }
}
