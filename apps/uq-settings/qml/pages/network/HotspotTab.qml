// Network's Hotspot tab: share this computer's connection.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    // Passed in from NetworkPage.qml.
    property var hotspotDialog
    property var nm
    property var page

    title: "Hotspot"
    iconName: "network-wireless-hotspot"
    available: page.ready && page.hotspot.can === true

    SettingsCard {
        title: "Hotspot"
        iconName: "network-wireless-hotspot"
        help: "While the hotspot is on, this computer's Wi-Fi isn't used to join other networks."
        wide: true

        SwitchRow {
            settingId: "hotspot"
            iconName: "preferences-system-network-sharing"
            label: page.hotspot.state === "on" ? "Sharing as " + String(page.hotspot.name || "") : "Share this computer's connection over Wi-Fi"
            highlight: App.highlight
            value: page.hotspot.state === "on" || page.hotspot.state === "starting"
            enabled: page.idle && page.wifi.enabled === true
            onChosen: on => {
                if (on)
                    hotspotDialog.open();
                else
                    nm.hotspot(false, "", "");
            }
            Layout.fillWidth: true
        }
    }
}
