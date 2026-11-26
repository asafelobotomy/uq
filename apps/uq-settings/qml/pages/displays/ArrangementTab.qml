// Displays' Arrangement tab: where each screen sits, its mode, scale and rotation.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    // Passed in from DisplaysPage.qml.
    property var dm
    property var page

    title: "Arrangement"
    iconName: "preferences-desktop-display-randr"

    // The hero: the screens as they sit. Click one to pick it, drag it to move it.
    SettingsCard {
        id: arrangementCard
        title: "Arrangement"
        iconName: "preferences-desktop-display-randr"
        help: page.view.length > 1 ? "Drag a display to move it. Click one to change its settings." : ""
        Layout.fillHeight: true

        SettingRow {
            settingId: "display-arrangement"
            label: ""
            highlight: App.highlight
            wideControls: true
            Layout.fillWidth: true
            Layout.fillHeight: true

            DisplayArrangement {
                objectName: "arrangement"
                outputs: page.view
                selected: page.current ? page.current.name : ""
                enabled: page.ready && !dm.busy
                onPicked: name => page.selected = name
                onMoved: (name, x, y) => dm.moveOutput(name, x, y)
                Layout.fillWidth: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 16
            }
        }

        // Every display, the ones off or mirrored too: pick one to change it.
        Flow {
            objectName: "displayPicker"
            visible: page.view.length > 1
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing

            QQC2.ButtonGroup {
                id: pickGroup
            }
            Repeater {
                model: page.view
                QQC2.Button {
                    id: pick
                    required property var modelData
                    checkable: true
                    checked: page.current !== null && page.current.name === modelData.name
                    QQC2.ButtonGroup.group: pickGroup
                    Accessible.name: String(modelData.label)
                    onClicked: page.selected = modelData.name
                    contentItem: RowLayout {
                        spacing: Kirigami.Units.smallSpacing
                        Kirigami.Icon {
                            source: pick.modelData.enabled ? "video-display" : "system-shutdown"
                            implicitWidth: Kirigami.Units.iconSizes.small
                            implicitHeight: Kirigami.Units.iconSizes.small
                            opacity: pick.modelData.enabled ? 1 : 0.5
                            Accessible.ignored: true
                        }
                        UqLabel {
                            text: String(pick.modelData.label)
                            font.bold: pick.checked
                            elide: Text.ElideRight
                            Layout.maximumWidth: Kirigami.Units.gridUnit * 14
                        }
                    }
                }
            }
        }
    }

    // The picked display's own settings, beside the picture.
    SettingsCard {
        objectName: "displayCard"
        title: page.current ? String(page.current.label) : "Display"
        iconName: "video-display"
        help: page.current ? "Connector " + page.current.name + (page.current.description ? ". " + page.current.description : "") : ""

        SwitchRow {
            settingId: "display-enabled"
            iconName: "system-shutdown"
            label: "Use this display"
            highlight: App.highlight
            enabled: page.ready && page.current !== null
            value: page.current ? page.current.enabled : true
            onChosen: v => page.edit("enabled", v ? null : false)
            Layout.fillWidth: true
        }
        SettingRow {
            settingId: "display-resolution"
            iconName: "view-fullscreen"
            label: "Resolution"
            highlight: App.highlight
            enabled: page.canEdit
            Layout.fillWidth: true

            UqComboBox {
                model: page.sizes.map(s => s.w + " × " + s.h)
                currentIndex: page.sizeIndex
                Accessible.name: "Resolution"
                onActivated: index => {
                    const s = page.sizes[index];
                    page.edit("mode", s.w + "x" + s.h + (s.rates.length ? "@" + s.rates[0] : ""));
                }
            }
        }
        SettingRow {
            settingId: "display-refresh"
            iconName: "speedometer"
            label: "Refresh rate"
            highlight: App.highlight
            enabled: page.canEdit && page.size.rates.length > 0
            Layout.fillWidth: true

            // A few rates show as buttons; a long list stays a menu.
            QQC2.ButtonGroup {
                id: rateGroup
            }
            Repeater {
                model: page.size.rates.length <= 4 ? page.size.rates : []
                ChoiceButton {
                    id: rateButton
                    required property real modelData
                    required property int index
                    objectName: "rate"
                    checkable: true
                    checked: index === page.rateIndex
                    QQC2.ButtonGroup.group: rateGroup
                    Accessible.name: page.rateText(modelData)
                    label: page.rateText(rateButton.modelData)
                    onClicked: {
                        page.edit("mode", page.size.w + "x" + page.size.h + "@" + modelData);
                        checked = Qt.binding(() => rateButton.index === page.rateIndex);
                    }
                }
            }
            UqComboBox {
                visible: page.size.rates.length > 4
                model: page.size.rates.map(r => page.rateText(r))
                currentIndex: page.rateIndex
                Accessible.name: "Refresh rate"
                onActivated: index => page.edit("mode", page.size.w + "x" + page.size.h + "@" + page.size.rates[index])
            }
        }
        SettingRow {
            settingId: "display-scale"
            iconName: "zoom-in"
            label: "Scale"
            help: "Bigger makes text and windows larger, with less on the screen."
            highlight: App.highlight
            enabled: page.canEdit
            Layout.fillWidth: true

            QQC2.ButtonGroup {
                id: scaleGroup
            }
            Repeater {
                model: page.scales.length <= 6 ? page.scales : []
                ChoiceButton {
                    id: scaleButton
                    required property real modelData
                    objectName: "scale"
                    checkable: true
                    checked: modelData === page.shownScale
                    QQC2.ButtonGroup.group: scaleGroup
                    Accessible.name: Math.round(modelData * 100) + "%"
                    label: Math.round(scaleButton.modelData * 100) + "%"
                    onClicked: {
                        page.edit("scale", modelData);
                        checked = Qt.binding(() => scaleButton.modelData === page.shownScale);
                    }
                }
            }
            UqComboBox {
                visible: page.scales.length > 6
                model: page.scales.map(s => Math.round(s * 100) + "%")
                currentIndex: Math.max(0, page.scales.indexOf(page.shownScale))
                Accessible.name: "Scale"
                onActivated: index => page.edit("scale", page.scales[index])
            }
        }
        SettingRow {
            settingId: "display-rotation"
            iconName: "object-rotate-right"
            label: "Rotation"
            highlight: App.highlight
            enabled: page.canEdit
            Layout.fillWidth: true

            QQC2.ButtonGroup {
                id: turnGroup
            }
            // Each button shows a screen turned the way it would be.
            Repeater {
                model: 4
                QQC2.Button {
                    id: turnButton
                    required property int index
                    objectName: "turn" + index
                    checkable: true
                    checked: index === page.turn
                    QQC2.ButtonGroup.group: turnGroup
                    Accessible.name: page.rotationNames[index]
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: page.rotationNames[index]
                    implicitWidth: Kirigami.Units.gridUnit * 2.5
                    implicitHeight: Kirigami.Units.gridUnit * 2.5
                    contentItem: Item {
                        Rectangle {
                            anchors.centerIn: parent
                            width: Kirigami.Units.gridUnit * 1.4
                            height: Kirigami.Units.gridUnit * 0.9
                            rotation: turnButton.index * 90
                            radius: 2
                            color: "transparent"
                            border.width: 2
                            border.color: Kirigami.Theme.textColor
                            // The screen's top edge, so the turn shows.
                            Rectangle {
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                height: 4
                                color: Kirigami.Theme.highlightColor
                            }
                        }
                    }
                    onClicked: {
                        page.setTransform(index, page.flipped);
                        checked = Qt.binding(() => turnButton.index === page.turn);
                    }
                }
            }
            QQC2.ToolButton {
                objectName: "flip"
                text: "Mirror image"
                icon.name: "object-flip-horizontal"
                display: QQC2.AbstractButton.IconOnly
                checkable: true
                checked: page.flipped
                Accessible.name: "Mirror image"
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: "Mirror image"
                onClicked: {
                    page.setTransform(page.turn, !page.flipped);
                    checked = Qt.binding(() => page.flipped);
                }
            }
        }
        SettingRow {
            settingId: "display-mirror"
            iconName: "view-split-left-right"
            label: "Show the same picture as"
            highlight: App.highlight
            enabled: page.canEdit && page.others.length > 0
            Layout.fillWidth: true

            UqComboBox {
                model: ["Don't mirror"].concat(page.others.map(o => o.label))
                currentIndex: page.cfg.mirror_of ? Math.max(0, page.others.findIndex(o => o.name === page.cfg.mirror_of) + 1) : 0
                Accessible.name: "Mirror"
                onActivated: index => page.edit("mirror_of", index === 0 ? null : page.others[index - 1].name)
            }
        }
        SettingRow {
            settingId: "display-vrr"
            iconName: "kt-speed-limits"
            label: "Variable refresh rate"
            help: "Lets the screen match a game's or video's frame rate, for smoother motion."
            highlight: App.highlight
            enabled: page.canEdit
            Layout.fillWidth: true

            UqComboBox {
                model: page.vrrNames
                currentIndex: page.cfg.vrr !== undefined ? page.cfg.vrr : 0
                Accessible.name: "Variable refresh rate"
                onActivated: index => page.edit("vrr", index === 0 ? null : index)
            }
        }
    }
}
