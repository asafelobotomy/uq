// Network's password dialog, shown when NetworkManager asks for a connection started here.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Kirigami.Dialog {
    id: secretDialog
    // Passed in from NetworkPage.qml.
    property var nm

    property int askId: -1
    property var question: ({})
    property bool answered: false

    function ask(q) {
        secretDialog.question = q;
        secretDialog.askId = q.id;
        secretDialog.answered = false;
        secretDialog.open();
    }
    function send() {
        const values = {};
        for (let i = 0; i < secretFields.count; i++) {
            const f = secretFields.itemAt(i);
            values[f.key] = f.field.text;
        }
        secretDialog.answered = true;
        nm.answerSecret(secretDialog.askId, JSON.stringify(values));
        secretDialog.close();
    }

    title: "Password needed"
    standardButtons: Kirigami.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 24
    onClosed: {
        for (let i = 0; i < secretFields.count; i++)
            secretFields.itemAt(i).field.wipe();
        if (!secretDialog.answered && secretDialog.askId >= 0)
            nm.cancelSecret(secretDialog.askId);
    }
    customFooterActions: [
        Kirigami.Action {
            text: "Connect"
            icon.name: "network-connect"
            onTriggered: secretDialog.send()
        }
    ]
    ColumnLayout {
        UqLabel {
            text: (secretDialog.question.retry ? "That didn't work. " : "") + "Type the password for the " + String(secretDialog.question.what || "network") + " " + String(secretDialog.question.name || "") + "."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        Repeater {
            id: secretFields
            model: secretDialog.question.fields || []
            delegate: ColumnLayout {
                required property var modelData
                readonly property string key: String(modelData.key)
                property alias field: secretField
                Layout.fillWidth: true
                UqLabel {
                    text: String(modelData.label)
                }
                SecretField {
                    id: secretField
                    Accessible.name: String(modelData.label)
                    Layout.fillWidth: true
                    onAccepted: secretDialog.send()
                }
            }
        }
    }
}
