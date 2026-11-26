// Network's dialog for joining a hidden network.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Kirigami.Dialog {
    id: hiddenDialog
    // Passed in from NetworkPage.qml.
    property var nm
    property var page

    title: "Join a hidden network"
    standardButtons: Kirigami.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 24
    onOpened: {
        hiddenName.clear();
        hiddenSecurity.currentIndex = 1;
        hiddenEveryone.checked = false;
        hiddenName.forceActiveFocus();
    }
    onClosed: hiddenPassword.wipe()
    customFooterActions: [
        Kirigami.Action {
            text: "Connect"
            icon.name: "network-connect"
            enabled: page.idle && hiddenName.text.trim() !== "" && (hiddenSecurity.currentIndex === 0 || hiddenPassword.text !== "")
            onTriggered: {
                nm.connectHidden(hiddenName.text, hiddenSecurity.currentIndex === 0 ? "open" : "password", hiddenPassword.text, hiddenEveryone.checked);
                hiddenDialog.close();
            }
        }
    ]
    GridLayout {
        columns: 2
        UqLabel {
            text: "Network name"
        }
        QQC2.TextField {
            id: hiddenName
            maximumLength: 32
            Accessible.name: "Network name"
            Layout.fillWidth: true
        }
        UqLabel {
            text: "Protection"
        }
        UqComboBox {
            id: hiddenSecurity
            model: ["None", "Password"]
            Accessible.name: "Protection"
            Layout.fillWidth: true
        }
        UqLabel {
            visible: hiddenSecurity.currentIndex === 1
            text: "Password"
        }
        SecretField {
            id: hiddenPassword
            visible: hiddenSecurity.currentIndex === 1
            Accessible.name: "Network password"
            Layout.fillWidth: true
        }
        Item {
            implicitWidth: 1
        }
        QQC2.CheckBox {
            id: hiddenEveryone
            text: "Everyone who uses this computer can join it too"
        }
    }
}
