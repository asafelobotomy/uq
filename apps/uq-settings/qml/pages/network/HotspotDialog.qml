// Network's hotspot dialog: the name and password of the shared network.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Kirigami.Dialog {
    id: hotspotDialog
    // Passed in from NetworkPage.qml.
    property var nm
    property var page

    title: "Share this computer's connection"
    standardButtons: Kirigami.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 24
    onOpened: {
        hotspotName.text = String(page.hotspot.name || "");
        hotspotName.forceActiveFocus();
    }
    onClosed: hotspotPassword.wipe()
    customFooterActions: [
        Kirigami.Action {
            text: "Turn on"
            icon.name: "network-wireless-hotspot"
            enabled: page.idle && (page.hotspot.uuid ? true : hotspotName.text.trim() !== "" && hotspotPassword.text.length >= 8)
            onTriggered: {
                nm.hotspot(true, hotspotName.text, hotspotPassword.text);
                hotspotDialog.close();
            }
        }
    ]
    GridLayout {
        columns: 2
        UqLabel {
            text: "Name"
        }
        QQC2.TextField {
            id: hotspotName
            maximumLength: 32
            Accessible.name: "Hotspot name"
            Layout.fillWidth: true
        }
        UqLabel {
            text: "Password"
        }
        SecretField {
            id: hotspotPassword
            Accessible.name: "Hotspot password, at least 8 characters"
            Layout.fillWidth: true
        }
        Item {
            implicitWidth: 1
        }
        UqLabel {
            text: page.hotspot.uuid ? "Leave the password empty to keep the saved one." : "At least 8 characters."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }
    }
}
