import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 400
    height: 200

    // What KeybindsModel.comboFromKey answers for the few keys the tests press.
    function stub(key, modifiers) {
        const meta = (modifiers & Qt.MetaModifier) !== 0;
        if (key === Qt.Key_Q)
            return meta ? "SUPER + Q" : "Q";
        if (key === Qt.Key_Space)
            return meta ? "SUPER + Space" : "Space";
        if (key === Qt.Key_Z)
            return (modifiers & Qt.ControlModifier) !== 0 ? "CTRL + Z" : "Z";
        return "";
    }

    Component {
        id: comp
        KeyCapture {
            width: 300
            comboFrom: root.stub
        }
    }
    Component {
        id: undoComp
        Item {
            property alias undo: undoShortcut
            width: 300
            height: 100
            property alias capture: kc
            KeyCapture {
                id: kc
                width: 300
                comboFrom: root.stub
            }
            Shortcut {
                id: undoShortcut
                sequence: "Ctrl+Z"
                context: Qt.WindowShortcut
            }
        }
    }
    Component {
        id: spyComp
        SignalSpy {}
    }

    TestCase {
        name: "KeyCapture"
        when: windowShown

        function make(props) {
            const c = createTemporaryObject(comp, root, props || {});
            verify(c);
            return c;
        }
        function spy(c, name) {
            return createTemporaryObject(spyComp, root, { target: c, signalName: name });
        }

        // The review's M1: a mouse click must not leave focus on the button (Space would click it),
        // and a window-level Ctrl+Z must not fire while listening.
        function test_mouse_start_then_super_space_and_ctrl_z_are_captured() {
            const host = createTemporaryObject(undoComp, root);
            verify(host);
            const c = host.capture;
            const got = spy(c, "captured");
            const fired = createTemporaryObject(spyComp, root, { target: host.undo, signalName: "activated" });
            const button = findChild(c, "captureButton");
            compare(button.focusPolicy, Qt.NoFocus);
            mouseClick(button);
            verify(c.listening);
            verify(c.activeFocus);
            keyPress(Qt.Key_Space, Qt.MetaModifier);
            compare(got.count, 1);
            compare(got.signalArguments[0][0], "SUPER + Space");
            verify(!c.listening);
            mouseClick(button);
            verify(c.listening);
            keyPress(Qt.Key_Z, Qt.ControlModifier);
            compare(got.count, 2);
            compare(got.signalArguments[1][0], "CTRL + Z");
            compare(fired.count, 0);
        }

        function test_a_combo_is_captured_and_listening_ends() {
            const c = make();
            const started = spy(c, "started");
            const ended = spy(c, "ended");
            const got = spy(c, "captured");
            verify(!c.listening);
            verify(!findChild(c, "captureNote").visible);
            c.start();
            verify(c.listening);
            compare(started.count, 1);
            verify(findChild(c, "captureNote").visible);
            compare(findChild(c, "captureNote").text, "Your shortcuts are paused while you press keys.");
            keyPress(Qt.Key_Space, Qt.MetaModifier);
            compare(got.count, 1);
            compare(got.signalArguments[0][0], "SUPER + Space");
            verify(!c.listening);
            compare(ended.count, 1, "the shortcuts are put back");
        }

        function test_a_modifier_alone_keeps_waiting() {
            const c = make();
            const got = spy(c, "captured");
            c.start();
            keyPress(Qt.Key_Meta, Qt.MetaModifier);
            keyRelease(Qt.Key_Meta);
            keyPress(Qt.Key_Shift, Qt.ShiftModifier);
            verify(c.listening);
            compare(got.count, 0);
            keyPress(Qt.Key_Q, Qt.MetaModifier);
            compare(got.signalArguments[0][0], "SUPER + Q");
        }

        function test_escape_alone_cancels() {
            const c = make();
            const got = spy(c, "captured");
            const ended = spy(c, "ended");
            c.start();
            keyPress(Qt.Key_Escape);
            verify(!c.listening);
            compare(got.count, 0);
            compare(ended.count, 1);
        }

        function test_listening_stops_by_itself() {
            const c = make({ timeout: 60 });
            const ended = spy(c, "ended");
            c.start();
            tryCompare(c, "listening", false, 2000);
            compare(ended.count, 1);
        }

        function test_losing_focus_ends_it() {
            const c = make();
            const ended = spy(c, "ended");
            c.start();
            c.focus = false;
            c.parent.forceActiveFocus();
            tryCompare(c, "listening", false, 2000);
            compare(ended.count, 1);
        }

        function test_unavailable_hides_it_and_never_starts() {
            const c = make({ available: false });
            const started = spy(c, "started");
            verify(!c.visible);
            c.start();
            verify(!c.listening);
            compare(started.count, 0);
        }

        function test_not_started_while_busy() {
            const c = make({ canStart: false });
            const started = spy(c, "started");
            c.start();
            compare(started.count, 0);
            verify(!findChild(c, "captureButton").enabled);
        }

        function test_button_starts_and_stops() {
            const c = make();
            const button = findChild(c, "captureButton");
            button.clicked();
            verify(c.listening);
            button.clicked();
            verify(!c.listening);
        }

        function test_keys_are_ignored_when_not_listening() {
            const c = make();
            const got = spy(c, "captured");
            c.forceActiveFocus();
            keyPress(Qt.Key_Q, Qt.MetaModifier);
            compare(got.count, 0);
        }
    }
}
