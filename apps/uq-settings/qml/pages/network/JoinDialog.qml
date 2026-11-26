// Network's dialog for joining a Wi-Fi network with a password.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Kirigami.Dialog {
    id: joinDialog
    // Passed in from NetworkPage.qml.
    property var nm
    property var page

    property var network: ({})
    title: "Join " + String(joinDialog.network.ssid || "")
    standardButtons: Kirigami.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 24
    onOpened: {
        joinEveryone.checked = false;
        joinPassword.forceActiveFocus();
    }
    onClosed: joinPassword.wipe()
    customFooterActions: [
        Kirigami.Action {
            text: "Connect"
            icon.name: "network-connect"
            enabled: page.idle && joinPassword.text !== ""
            onTriggered: {
                nm.connectWifi(String(joinDialog.network.ap), String(joinDialog.network.device), joinPassword.text, joinEveryone.checked);
                joinDialog.close();
            }
        }
    ]
    ColumnLayout {
        UqLabel {
            text: joinDialog.network.security === "old" ? "This network's key" : "This network's password"
        }
        SecretField {
            id: joinPassword
            objectName: "joinPassword"
            Accessible.name: "Network password"
            Layout.fillWidth: true
            onAccepted: if (text !== "")
                joinDialog.customFooterActions[0].trigger()
        }
        QQC2.CheckBox {
            id: joinEveryone
            text: "Everyone who uses this computer can join it too"
        }
    }
}
