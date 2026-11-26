// Bluetooth's pairing question: a code to confirm or type, or a yes/no.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Kirigami.Dialog {
    id: questionDialog
    // Passed in from BluetoothPage.qml.
    property var bm

    property int askId: -1
    property var q: ({})
    property bool answered: false
    readonly property bool typed: q.kind === "pin" || q.kind === "passkey"

    function ask(question) {
        questionDialog.q = question;
        questionDialog.askId = question.id;
        questionDialog.answered = false;
        typedCode.clear();
        questionDialog.open();
        if (questionDialog.typed)
            typedCode.forceActiveFocus();
    }
    function reply(yes) {
        questionDialog.answered = true;
        bm.answer(questionDialog.askId, yes, yes ? typedCode.text : "");
        questionDialog.close();
    }

    title: questionDialog.q.kind === "service" ? "Allow this device?" : "Pair with " + String(questionDialog.q.device || "") + "?"
    standardButtons: QQC2.Dialog.NoButton
    preferredWidth: Kirigami.Units.gridUnit * 24
    closePolicy: QQC2.Popup.CloseOnEscape
    onClosed: {
        if (!questionDialog.answered && questionDialog.askId >= 0)
            bm.answer(questionDialog.askId, false, "");
        typedCode.clear();
    }
    customFooterActions: [
        Kirigami.Action {
            text: "Confirm"
            icon.name: "dialog-ok"
            enabled: !questionDialog.typed || typedCode.text.trim() !== ""
            onTriggered: questionDialog.reply(true)
        },
        Kirigami.Action {
            text: "Reject"
            icon.name: "dialog-cancel"
            onTriggered: questionDialog.reply(false)
        }
    ]
    ColumnLayout {
        UqLabel {
            objectName: "questionText"
            text: {
                const dev = String(questionDialog.q.device || "the device");
                switch (questionDialog.q.kind) {
                case "confirm":
                    return "Check that " + dev + " shows this code, then press Confirm. If it shows a different code, press Reject.";
                case "authorize":
                    return dev + " wants to pair with this computer. Pair only with devices you know.";
                case "service":
                    return dev + " wants to " + String(questionDialog.q.service || "use a service") + ".";
                case "pin":
                    return "Type the PIN for " + dev + ". It's often in the device's manual (for example 0000).";
                case "passkey":
                    return "Type the 6-digit code " + dev + " shows.";
                default:
                    return "";
                }
            }
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "questionCode"
            visible: questionDialog.q.kind === "confirm"
            text: String(questionDialog.q.code || "")
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 2.5
            font.bold: true
            font.letterSpacing: Kirigami.Units.smallSpacing
            Layout.alignment: Qt.AlignHCenter
            Accessible.name: "Code " + String(questionDialog.q.code || "").split("").join(" ")
        }
        QQC2.TextField {
            id: typedCode
            visible: questionDialog.typed
            maximumLength: questionDialog.q.kind === "passkey" ? 6 : 16
            inputMethodHints: questionDialog.q.kind === "passkey" ? Qt.ImhDigitsOnly : Qt.ImhNone
            Accessible.name: questionDialog.q.kind === "passkey" ? "Code" : "PIN"
            Layout.fillWidth: true
            onAccepted: if (text.trim() !== "")
                questionDialog.reply(true)
        }
    }
}
