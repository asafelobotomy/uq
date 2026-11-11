import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 700
    height: 700

    // What the page's snapshot offers (uQ's name table).
    readonly property var choices: ({
        fingers: [3, 4, 5],
        directions: [
            { id: "horizontal", label: "Swipe sideways" },
            { id: "vertical", label: "Swipe up or down" },
            { id: "left", label: "Swipe left" },
            { id: "right", label: "Swipe right" },
            { id: "up", label: "Swipe up" },
            { id: "down", label: "Swipe down" },
            { id: "pinch_in", label: "Pinch in" },
            { id: "pinch_out", label: "Pinch out" }
        ],
        actions: [
            { id: "workspace", label: "Switch workspace", directions: ["horizontal", "vertical"] },
            { id: "move", label: "Move the window", directions: ["horizontal", "vertical", "left", "right", "up", "down"] },
            { id: "special", label: "Show or hide the scratch workspace", directions: ["horizontal", "vertical", "left", "right", "up", "down", "pinch_in", "pinch_out"] },
            { id: "close", label: "Close the window", directions: ["horizontal", "vertical", "left", "right", "up", "down", "pinch_in", "pinch_out"] }
        ],
        modifiers: [
            { id: "", label: "None" },
            { id: "SUPER", label: "Super" },
            { id: "ALT", label: "Alt" },
            { id: "CTRL", label: "Ctrl" },
            { id: "SHIFT", label: "Shift" }
        ]
    })
    readonly property string clashText: "“Close the window” and “Switch workspace” both use 3 fingers, swipe left."

    Component {
        id: comp
        GestureEditor {
            choices: root.choices
            // 3 fingers, swipe left clashes with uQ's sideways swipe.
            check: (index, json) => {
                const g = JSON.parse(json);
                return g.fingers === 3 && g.direction === "left" && !g.modifier ? root.clashText : "";
            }
        }
    }

    Component {
        id: spyComp
        SignalSpy {}
    }

    TestCase {
        name: "GestureEditor"
        when: windowShown

        function make(props) {
            const d = createTemporaryObject(comp, root, props || {});
            verify(d);
            return d;
        }
        function opened(props) {
            const d = make(props);
            d.startNew();
            tryCompare(d, "opened", true);
            return d;
        }
        function spy(d, name) {
            const s = createTemporaryObject(spyComp, root, { target: d, signalName: name });
            verify(s.valid, name);
            return s;
        }
        function ids(list) {
            return list.map(d => d.id);
        }

        function test_conflict_warning_shown_and_save_disabled() {
            const d = opened();
            verify(d.saveEnabled, "the default gesture on its own is fine");
            verify(!d.warningMessage.visible);
            d.action = "close";
            d.direction = "left";
            compare(d.warning, root.clashText);
            verify(d.warningMessage.visible);
            compare(findChild(d.warningMessage, "messageLine0").text, root.clashText);
            verify(!d.saveEnabled);
            verify(!d.saveButton.enabled);
            // Save does nothing while it clashes.
            const got = spy(d, "gestureAccepted");
            d.save();
            keyClick(Qt.Key_Return);
            compare(got.count, 0);
        }

        function test_warning_clears_when_fixed() {
            const d = opened();
            d.action = "close";
            d.direction = "left";
            verify(!d.saveEnabled);
            d.fingers = 4;
            compare(d.warning, "");
            verify(!d.warningMessage.visible);
            verify(d.saveEnabled);
            verify(d.saveButton.enabled);
            // A key held down makes it a different gesture too.
            d.fingers = 3;
            verify(!d.saveEnabled);
            d.modifier = "SUPER";
            verify(d.saveEnabled);
        }

        function test_directions_follow_action() {
            const d = opened();
            compare(ids(d.directions), ["horizontal", "vertical"]);
            compare(d.directionBox.model.length, 2);
            d.action = "move";
            compare(d.directions.length, 6);
            verify(ids(d.directions).indexOf("pinch_in") < 0);
            d.action = "close";
            compare(d.directions.length, 8);
            d.direction = "pinch_in";
            d.action = "workspace";
            compare(d.direction, "horizontal", "a direction the action can't use is replaced");
            d.action = "move";
            d.direction = "left";
            d.action = "workspace";
            compare(d.direction, "horizontal");
            verify(d.saveEnabled);
        }

        function test_special_name_only_for_special() {
            const d = opened();
            verify(!d.nameField.visible);
            verify(!findChild(d, "nameLabel").visible);
            d.action = "special";
            verify(d.nameField.visible);
            verify(findChild(d, "nameLabel").visible);
            d.nameField.text = "bad name!";
            verify(!d.nameOk);
            verify(findChild(d, "nameHint").visible);
            verify(!d.saveEnabled);
            d.nameField.text = "notes";
            verify(d.nameOk);
            verify(d.saveEnabled);
            compare(d.draft().workspace, "notes");
            d.nameField.text = "";
            verify(d.saveEnabled, "empty means the usual name");
            compare(d.draft().workspace, undefined);
            d.nameField.text = "notes";
            d.action = "close";
            verify(!d.nameField.visible);
            compare(d.draft().workspace, undefined, "the name goes with the action");
        }

        function test_escape_cancels_enter_saves() {
            const d = opened();
            const got = spy(d, "gestureAccepted");
            keyClick(Qt.Key_Escape);
            tryCompare(d, "opened", false);
            compare(got.count, 0);
            d.startNew();
            tryCompare(d, "opened", true);
            d.fingers = 4;
            d.action = "close";
            d.direction = "up";
            keyClick(Qt.Key_Return);
            compare(got.count, 1);
            compare(got.signalArguments[0][0], -1);
            const sent = JSON.parse(got.signalArguments[0][1]);
            compare(sent.fingers, 4);
            compare(sent.direction, "up");
            compare(sent.action, "close");
            compare(sent.enabled, true);
            verify(d.opened, "it waits for uQ's answer");
            d.finishSave("");
            tryCompare(d, "opened", false);
        }

        function test_refused_save_stays_open_and_says_why() {
            const d = opened();
            d.save();
            verify(d.saving);
            d.finishSave("fingers is a whole number from 3 to 5");
            verify(d.opened);
            verify(!d.saving);
            compare(findChild(d, "saveError").text, "That wasn't saved: fingers is a whole number from 3 to 5");
            compare(findChild(d, "saveError").textFormat, Text.PlainText);
        }

        function test_change_fills_in_the_gesture() {
            const d = make();
            d.change(1, { fingers: 4, direction: "up", action: "close", modifier: "SUPER", enabled: false });
            tryCompare(d, "opened", true);
            compare(d.title, "Change gesture");
            compare(d.fingersBox.currentIndex, 1);
            compare(d.actionBox.currentIndex, 3);
            compare(d.modifierBox.currentText, "Super");
            compare(d.directionBox.currentText, "Swipe up");
            const got = spy(d, "gestureAccepted");
            d.save();
            compare(got.signalArguments[0][0], 1);
            const sent = JSON.parse(got.signalArguments[0][1]);
            compare(sent.modifier, "SUPER");
            compare(sent.enabled, false, "changing it doesn't turn it on");
            d.startNew();
            compare(d.title, "Add a gesture");
        }

        function test_busy_blocks_save() {
            const d = opened({ canSave: false });
            verify(!d.saveEnabled);
            const got = spy(d, "gestureAccepted");
            keyClick(Qt.Key_Return);
            compare(got.count, 0);
        }

        function test_choices_are_plain_text() {
            const d = opened();
            compare(d.fingersBox.model[0], "3 fingers");
            compare(d.fingersBox.contentItem.textFormat, Text.PlainText);
        }
    }
}
