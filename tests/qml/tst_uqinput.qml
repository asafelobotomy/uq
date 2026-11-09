import QtQuick
import QtTest
import "../../shell/core"
import "../../shell/logic/keys.js" as K

TestCase {
    name: "uqinput"
    width: 400; height: 100
    when: windowShown

    Component { id: c; UqInput {} }

    function inner(f) {
        for (var i = 0; i < f.children.length; i++)
            if (f.children[i].echoMode !== undefined)
                return f.children[i];
        return null;
    }

    function test_plain_default() {
        var f = createTemporaryObject(c, this);
        var t = inner(f);
        verify(t !== null);
        compare(f.secret, false);
        compare(t.echoMode, TextInput.Normal);
        compare(t.inputMethodHints, Qt.ImhNoPredictiveText);
        compare(f.maximumLength, 100);
    }

    function test_secret_mode() {
        var f = createTemporaryObject(c, this, { secret: true });
        var t = inner(f);
        compare(t.echoMode, TextInput.Password);
        compare(t.inputMethodHints, Qt.ImhSensitiveData | Qt.ImhNoPredictiveText | Qt.ImhHiddenText);
        verify(f.maximumLength >= 512);
    }

    function test_accepted_and_clear() {
        var f = createTemporaryObject(c, this, { secret: true });
        var spy = createTemporaryObject(spyC, this, { target: f, signalName: "accepted" });
        f.forceActiveFocus();
        keyClick(Qt.Key_A);
        keyClick(Qt.Key_B);
        compare(f.text, "ab");
        keyClick(Qt.Key_Return);
        compare(spy.count, 1);
        f.clear();
        compare(f.text, "");
    }

    // Shortcuts pressed over the lock (SUPER+Space, SUPER+N) must not type into its password field.
    function test_secret_ignores_shortcuts() {
        var f = createTemporaryObject(c, this, { secret: true });
        f.forceActiveFocus();
        keyClick(Qt.Key_A);
        keyClick(Qt.Key_Space, Qt.MetaModifier);
        keyClick(Qt.Key_N, Qt.MetaModifier);
        keyClick(Qt.Key_N, Qt.MetaModifier | Qt.ShiftModifier);
        keyClick(Qt.Key_B);
        compare(f.text, "ab");
        // Editing combos still work: ctrl+a then Backspace clears the field.
        keyClick(Qt.Key_A, Qt.ControlModifier);
        keyClick(Qt.Key_Backspace);
        compare(f.text, "");
        keyClick(Qt.Key_C);
        compare(f.text, "c");
    }

    function test_block_key() {
        compare(K.blocks(Qt.MetaModifier, Qt.Key_Space), true);
        compare(K.blocks(Qt.MetaModifier, Qt.Key_N), true);
        compare(K.blocks(Qt.MetaModifier | Qt.ShiftModifier, Qt.Key_N), true);
        compare(K.blocks(Qt.MetaModifier, Qt.Key_Backspace), true);
        compare(K.blocks(Qt.NoModifier, Qt.Key_N), false);
        compare(K.blocks(Qt.ShiftModifier, Qt.Key_N), false);
        compare(K.blocks(Qt.KeypadModifier, Qt.Key_5), false);
        // Ctrl: only the editing combos get through.
        for (var k of [Qt.Key_A, Qt.Key_Backspace, Qt.Key_Delete, Qt.Key_Left, Qt.Key_Right,
                       Qt.Key_Home, Qt.Key_End, Qt.Key_Z, Qt.Key_Y, Qt.Key_X, Qt.Key_C, Qt.Key_V,
                       Qt.Key_Return, Qt.Key_Enter])
            compare(K.blocks(Qt.ControlModifier, k), false, "ctrl+" + k);
        compare(K.blocks(Qt.ControlModifier | Qt.ShiftModifier, Qt.Key_Left), false);
        compare(K.blocks(Qt.ControlModifier, Qt.Key_N), true);
        compare(K.blocks(Qt.ControlModifier, Qt.Key_Space), true);
        // Ctrl+Alt is AltGr on some layouts: it types.
        compare(K.blocks(Qt.ControlModifier | Qt.AltModifier, Qt.Key_Q), false);
        compare(K.blocks(Qt.AltModifier, Qt.Key_Q), false);
    }

    Component { id: spyC; SignalSpy {} }
}
