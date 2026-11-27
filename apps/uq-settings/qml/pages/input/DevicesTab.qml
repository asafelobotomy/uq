// Input's Devices tab: settings for one mouse or touchpad at a time.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    // Passed in from InputPage.qml.
    property var im
    property var page

    title: "Devices"
    iconName: "preferences-desktop-peripherals"

    SettingsCard {
        title: "Settings for one device"
        iconName: "preferences-desktop-peripherals"
        help: "A device's own settings win over the mouse's. A new one starts from the mouse's speed."
        wide: Object.keys(page.deviceSettings).length === 0

        SettingRow {
            settingId: "device-overrides"
            iconName: "list-add"
            label: "Add a device"
            highlight: App.highlight
            enabled: page.ready && page.addable.length > 0
            Layout.fillWidth: true

            UqComboBox {
                id: devicePick
                model: ["Choose a device"].concat(page.addable.map(p => p.name))
                currentIndex: 0
                Accessible.name: "Device to add"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 14
            }
            QQC2.Button {
                objectName: "addDevice"
                text: "Add"
                icon.name: "list-add"
                enabled: devicePick.currentIndex > 0 && !im.busy
                onClicked: {
                    // Starts from the mouse's own speed, so nothing moves until you change it.
                    im.addDevice(page.addable[devicePick.currentIndex - 1].name);
                    devicePick.currentIndex = 0;
                }
            }
        }
        UqLabel {
            visible: page.pointers.length === 0
            text: "No pointing devices found."
            opacity: 0.7
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
    }

    Repeater {
        model: Object.keys(page.deviceSettings)

        SettingsCard {
            id: dev
            required property string modelData
            readonly property var s: page.deviceSettings[modelData] || {}
            title: modelData
            iconName: modelData.indexOf("touchpad") >= 0 ? "input-touchpad" : "input-mouse"
            actions: [
                QQC2.ToolButton {
                    text: "Remove"
                    icon.name: "edit-delete"
                    display: QQC2.AbstractButton.IconOnly
                    enabled: page.ready && !im.busy
                    Accessible.name: "Remove " + dev.modelData
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: "Remove this device's settings"
                    onClicked: im.removeDevice(dev.modelData)
                }
            ]

            OverrideSlider {
                busy: im.busy
                iconName: "speedometer"
                label: "Pointer speed"
                enabled: page.ready
                from: -1
                to: 1
                fallback: page.mouse.sensitivity || 0
                value: page.saved(dev.s, "sensitivity")
                onChosen: v => page.setDevice(dev.modelData, "sensitivity", v)
                Layout.fillWidth: true
            }
            OverrideCombo {
                iconName: "transform-scale"
                label: "Pointer acceleration"
                enabled: page.ready
                themeLabel: "Same as the mouse"
                choices: page.accelCodes
                labels: ({ "default": "Default", "adaptive": "Adaptive", "flat": "Flat" })
                value: page.saved(dev.s, "accel_profile")
                onChosen: v => page.setDevice(dev.modelData, "accel_profile", v)
                Layout.fillWidth: true
            }
            OverrideSwitch {
                iconName: "transform-browse"
                label: "Natural scrolling"
                enabled: page.ready
                fallback: page.mouse.natural_scroll === true
                value: page.saved(dev.s, "natural_scroll")
                onChosen: v => page.setDevice(dev.modelData, "natural_scroll", v)
                Layout.fillWidth: true
            }
            OverrideSwitch {
                iconName: "object-flip-horizontal"
                label: "Left-handed buttons"
                enabled: page.ready
                fallback: page.mouse.left_handed === true
                value: page.saved(dev.s, "left_handed")
                onChosen: v => page.setDevice(dev.modelData, "left_handed", v)
                Layout.fillWidth: true
            }
            OverrideSlider {
                busy: im.busy
                iconName: "arrow-down"
                label: "Scroll speed"
                enabled: page.ready
                from: 0.1
                to: 5
                stepSize: 0.1
                fallback: page.mouse.scroll_factor || 1
                value: page.saved(dev.s, "scroll_factor")
                onChosen: v => page.setDevice(dev.modelData, "scroll_factor", v)
                Layout.fillWidth: true
            }
        }
    }
}
