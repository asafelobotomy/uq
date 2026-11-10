// "Keep these display settings?" while a display change is being tried. It counts down from
// `seconds`; at 0 it only says "Reverting…", because the settings service puts the old layout
// back by itself (even if this app has crashed). Neither button is the default, so Enter does
// nothing; Escape or closing the dialog reverts. Plain controls only, so it works on any screen.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts

QQC2.Dialog {
    id: dialog

    /// Seconds left when the change was applied (from the page model).
    property int seconds: 15
    /// Shown while a change is being tried.
    property bool active: false
    /// Seconds left on the countdown shown.
    property int left: seconds
    readonly property string message: left > 0 ? "Reverting in " + left + (left === 1 ? " second." : " seconds.") : "Reverting…"
    readonly property alias keepButton: keepBtn
    readonly property alias revertButton: revertBtn

    signal keep()
    signal revert()

    title: "Keep these display settings?"
    modal: true
    closePolicy: QQC2.Popup.CloseOnEscape
    anchors.centerIn: parent
    standardButtons: QQC2.Dialog.NoButton
    width: Math.min(420, parent ? parent.width : 420)

    onActiveChanged: {
        if (active) {
            left = Math.max(0, seconds);
            countdown.restart();
            open();
        } else {
            countdown.stop();
            close();
        }
    }
    onSecondsChanged: left = Math.max(0, seconds)
    // Escape (or the window manager closing it) means "no": put the old layout back.
    onRejected: {
        if (active)
            revert();
    }

    Timer {
        id: countdown
        interval: 1000
        repeat: true
        onTriggered: {
            dialog.left = Math.max(0, dialog.left - 1);
            if (dialog.left === 0)
                stop();
        }
    }

    contentItem: ColumnLayout {
        focus: true
        Accessible.role: Accessible.Dialog
        Accessible.name: dialog.title
        Accessible.description: dialog.message
        // Swallow Enter: keeping must be a deliberate click on Keep changes.
        Keys.onReturnPressed: event => event.accepted = true
        Keys.onEnterPressed: event => event.accepted = true

        UqLabel {
            objectName: "countdown"
            text: dialog.message
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
    }

    footer: QQC2.DialogButtonBox {
        QQC2.Button {
            id: keepBtn
            text: "Keep changes"
            onClicked: dialog.keep()
        }
        QQC2.Button {
            id: revertBtn
            text: "Revert"
            onClicked: dialog.revert()
        }
    }
}
