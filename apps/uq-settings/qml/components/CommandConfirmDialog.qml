// "Are you sure?" before a plugin page's command runs (Phase 4 spec §2.3 `confirm`). The question
// is the page's fixed text from page.toml, shown as plain text. The command runs only on an
// explicit Confirm; Cancel, Escape or closing the dialog cancels it. Neither button is the
// default, so Enter does nothing. Plain controls only, so it works without Kirigami.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts

QQC2.Dialog {
    id: dialog

    /// The request waiting for an answer; -1 when none.
    property int request: -1
    /// The question, from page.toml (plain text).
    property string question
    readonly property alias confirmButton: confirmBtn
    readonly property alias cancelButton: cancelBtn
    readonly property alias questionLabel: questionText

    /// The answer for `request`: true only for Confirm.
    signal answered(int request, bool confirmed)

    /// Ask about `request`. A question still open is cancelled first.
    function ask(req, text) {
        if (dialog.request >= 0)
            dialog.reply(false);
        dialog.request = req;
        dialog.question = text;
        dialog.open();
    }

    function reply(confirmed) {
        const req = dialog.request;
        if (req < 0)
            return;
        dialog.request = -1;
        dialog.answered(req, confirmed);
        if (dialog.opened)
            dialog.close();
    }

    title: "Are you sure?"
    modal: true
    closePolicy: QQC2.Popup.CloseOnEscape
    anchors.centerIn: parent
    standardButtons: QQC2.Dialog.NoButton
    width: Math.min(420, parent ? parent.width : 420)

    // Escape, or the window closing it, is "no".
    onClosed: dialog.reply(false)

    contentItem: ColumnLayout {
        spacing: 12

        UqLabel {
            id: questionText
            objectName: "confirmQuestion"
            text: dialog.question
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        RowLayout {
            Layout.alignment: Qt.AlignRight
            QQC2.Button {
                id: cancelBtn
                objectName: "confirmCancel"
                text: "Cancel"
                onClicked: dialog.reply(false)
            }
            QQC2.Button {
                id: confirmBtn
                objectName: "confirmRun"
                text: "Confirm"
                onClicked: dialog.reply(true)
            }
        }
    }
}
