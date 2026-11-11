import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

// Password boxes (spec §3): hidden, kept from the input method's prediction and learning, and
// emptied by wipe() after each attempt.
TestCase {
    id: tc
    name: "SecretField"
    visible: true
    when: windowShown
    width: 300
    height: 100

    Component {
        id: comp
        SecretField {}
    }

    function test_hidden_and_not_learned() {
        const f = createTemporaryObject(comp, tc);
        verify(f);
        compare(f.echoMode, TextInput.Password);
        verify(f.inputMethodHints & Qt.ImhSensitiveData);
        verify(f.inputMethodHints & Qt.ImhNoPredictiveText);
        verify(f.inputMethodHints & Qt.ImhHiddenText);
        verify(f.Accessible.passwordEdit);
    }

    function test_wipe_empties_the_box() {
        const f = createTemporaryObject(comp, tc);
        f.forceActiveFocus();
        keyClick(Qt.Key_S);
        keyClick(Qt.Key_3);
        compare(f.text, "s3");
        f.wipe();
        compare(f.text, "");
    }
}
