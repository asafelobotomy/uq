// Input's Mouse tab: speed, acceleration, buttons and scrolling.
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

    title: "Mouse"
    iconName: "input-mouse"

    SettingsCard {
        title: "Pointer"
        iconName: "tool-pointer"

        ValueSlider {
            id: speedRow
            busy: im.busy
            settingId: "mouse-speed"
            iconName: "speedometer"
            label: "Pointer speed"
            highlight: App.highlight
            enabled: page.ready
            source: page.sourceOf("mouse-speed")
            from: -1
            to: 1
            lowLabel: "Slower"
            highLabel: "Faster"
            value: page.mouse.sensitivity || 0
            onChosen: v => page.setMouse("sensitivity", v)
            Layout.fillWidth: true

            UqLabel {
                objectName: "speedValue"
                text: page.signed(speedRow.slider.value)
                horizontalAlignment: Text.AlignRight
                Layout.minimumWidth: Kirigami.Units.gridUnit * 2
            }
        }
        SettingRow {
            settingId: "mouse-accel"
            iconName: "transform-scale"
            label: "Pointer acceleration"
            help: "Adaptive speeds the pointer up on fast moves. Flat keeps it in step with your hand."
            highlight: App.highlight
            enabled: page.ready
            source: page.sourceOf("mouse-accel")
            Layout.fillWidth: true

            QQC2.ButtonGroup {
                id: accelGroup
            }
            Repeater {
                model: page.accelCodes
                ChoiceButton {
                    id: accelButton
                    required property string modelData
                    required property int index
                    objectName: "accel-" + modelData
                    checkable: true
                    checked: (page.mouse.accel_profile || "default") === modelData
                    QQC2.ButtonGroup.group: accelGroup
                    Accessible.name: page.accelNames[index]
                    label: page.accelNames[accelButton.index]
                    onClicked: {
                        page.setMouse("accel_profile", modelData);
                        checked = Qt.binding(() => (page.mouse.accel_profile || "default") === modelData);
                    }
                }
            }
        }
    }

    SettingsCard {
        title: "Buttons and wheel"
        iconName: "input-mouse"

        SwitchRow {
            settingId: "mouse-natural-scroll"
            iconName: "transform-browse"
            label: "Natural scrolling"
            help: "The page follows your fingers, as on a phone."
            highlight: App.highlight
            enabled: page.ready
            source: page.sourceOf("mouse-natural-scroll")
            value: page.mouse.natural_scroll === true
            onChosen: v => page.setMouse("natural_scroll", v)
            Layout.fillWidth: true
        }
        SwitchRow {
            settingId: "mouse-left-handed"
            iconName: "object-flip-horizontal"
            label: "Left-handed buttons"
            highlight: App.highlight
            enabled: page.ready
            source: page.sourceOf("mouse-left-handed")
            value: page.mouse.left_handed === true
            onChosen: v => page.setMouse("left_handed", v)
            Layout.fillWidth: true
        }
        ValueSlider {
            id: scrollRow
            busy: im.busy
            settingId: "mouse-scroll-speed"
            iconName: "arrow-down"
            label: "Scroll speed"
            highlight: App.highlight
            enabled: page.ready
            source: page.sourceOf("mouse-scroll-speed")
            from: 0.1
            to: 5
            stepSize: 0.1
            lowLabel: "Slower"
            highLabel: "Faster"
            value: page.mouse.scroll_factor || 1
            onChosen: v => page.setMouse("scroll_factor", v)
            Layout.fillWidth: true

            UqLabel {
                objectName: "scrollValue"
                text: "×" + (Math.round(scrollRow.slider.value * 10) / 10)
                horizontalAlignment: Text.AlignRight
                Layout.minimumWidth: Kirigami.Units.gridUnit * 2
            }
        }
    }
}
