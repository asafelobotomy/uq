// A password box (spec §3): hidden text, no prediction or learning by the input method, and
// cleared with wipe() after every attempt, whether it worked or not.
import QtQuick
import QtQuick.Controls as QQC2

QQC2.TextField {
    id: field

    /// Empty the box (after each attempt, and when its dialog closes).
    function wipe() {
        field.clear();
    }

    echoMode: TextInput.Password
    inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText | Qt.ImhHiddenText
    selectByMouse: false
    Accessible.passwordEdit: true
}
