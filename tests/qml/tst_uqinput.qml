import QtQuick
import QtTest
import "../../shell/core"

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

    Component { id: spyC; SignalSpy {} }
}
