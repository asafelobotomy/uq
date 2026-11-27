// Look's Layouts tab: how windows are arranged, per workspace.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    // Passed in from LookPage.qml.
    property var lm
    property var page

    title: "Layouts"
    iconName: "view-grid"

    SettingsCard {
        title: "Default layout"
        iconName: "view-grid"
        help: "How windows are arranged on workspaces without a layout of their own."

        // The three layouts drawn as tiles; "Use default" follows uQ's default.
        SettingRow {
            settingId: "default-layout"
            source: page.sourceOf("default-layout")
            onReapply: lm.reapply("layouts")
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            QQC2.ButtonGroup {
                id: layoutGroup
            }
            Repeater {
                model: page.layoutNames
                QQC2.Button {
                    required property string modelData
                    readonly property bool current: page.currentLayout === modelData
                    objectName: "layout-" + modelData
                    checkable: true
                    checked: current
                    QQC2.ButtonGroup.group: layoutGroup
                    Accessible.name: page.layoutLabels[modelData]
                    Accessible.description: page.layoutHelp[modelData]
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: page.layoutHelp[modelData]
                    contentItem: ColumnLayout {
                        spacing: Kirigami.Units.smallSpacing
                        LayoutDrawing {
                            layout: modelData
                            shapes: page.layoutShapes[modelData] || []
                            active: current
                            Layout.alignment: Qt.AlignHCenter
                        }
                        UqLabel {
                            text: page.layoutLabels[modelData]
                            font.bold: current
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                        }
                    }
                    onClicked: {
                        page.setOption("", "default", modelData);
                        checked = Qt.binding(() => current);
                    }
                }
            }
            ColumnLayout {
                spacing: 0
                Layout.alignment: Qt.AlignVCenter
                Layout.leftMargin: Kirigami.Units.smallSpacing
                QQC2.CheckBox {
                    id: layoutDefaultBox
                    objectName: "layoutUseDefault"
                    text: "Use default"
                    checked: !page.layouts.default
                    enabled: page.ready
                    onToggled: {
                        page.setOption("", "default", checked ? null : page.currentLayout);
                        checked = Qt.binding(() => !page.layouts.default);
                    }
                }
            }
        }
    }

    // Each layout's options, shown when the default or a workspace rule uses the layout (or
    // a search hit points at them).
    SettingsCard {
        title: "Dwindle"
        iconName: "view-split-left-right"
        visible: dwindleGroup.shown

        OptionGroup {
            id: dwindleGroup
            settingId: "dwindle-options"
            source: page.sourceOf("dwindle-options")
            onReapply: lm.reapply("layouts")
            label: "Dwindle layout options"
            highlight: App.highlight
            enabled: page.ready
            inUse: page.inUse.indexOf("dwindle") >= 0
            Layout.fillWidth: true
            OverrideSwitch {
                iconName: "object-flip-horizontal"
                label: "Keep the split direction"
                help: "A window keeps its split when the windows around it change."
                enabled: page.ready
                fallback: !!page.dGroup("dwindle").preserve_split
                value: page.saved(page.dwindle, "preserve_split")
                onChosen: v => page.setOption("dwindle", "preserve_split", v)
                Layout.fillWidth: true
            }
            OverrideSwitch {
                iconName: "transform-move"
                label: "Smart split"
                help: "Split by where the mouse is over the window."
                enabled: page.ready
                fallback: !!page.dGroup("dwindle").smart_split
                value: page.saved(page.dwindle, "smart_split")
                onChosen: v => page.setOption("dwindle", "smart_split", v)
                Layout.fillWidth: true
            }
            OverrideCombo {
                iconName: "window-new"
                label: "Where new windows go"
                enabled: page.ready
                themeLabel: "Use default"
                choices: Object.keys(page.splitLabels)
                labels: page.splitLabels
                value: page.dwindle.force_split || null
                onChosen: v => page.setOption("dwindle", "force_split", v)
                Layout.fillWidth: true
            }
        }
    }
    SettingsCard {
        title: "Master"
        iconName: "view-split-left-right"
        visible: masterGroup.shown

        OptionGroup {
            id: masterGroup
            settingId: "master-options"
            source: page.sourceOf("master-options")
            onReapply: lm.reapply("layouts")
            label: "Master layout options"
            highlight: App.highlight
            enabled: page.ready
            inUse: page.inUse.indexOf("master") >= 0
            Layout.fillWidth: true
            OverrideSlider {
                busy: lm.busy
                iconName: "transform-scale"
                label: "Master area share"
                enabled: page.ready
                from: 0.1
                to: 0.9
                fallback: page.num(page.dGroup("master").mfact)
                value: page.saved(page.master, "mfact")
                onChosen: v => page.setOption("master", "mfact", v)
                Layout.fillWidth: true
            }
            OverrideCombo {
                iconName: "window-new"
                label: "New windows join as"
                enabled: page.ready
                themeLabel: "Use default"
                choices: ["master", "slave", "inherit"]
                labels: ({ master: "Master", slave: "Stack", inherit: "Same as the focused window" })
                value: page.master.new_status || null
                onChosen: v => page.setOption("master", "new_status", v)
                Layout.fillWidth: true
            }
            OverrideCombo {
                iconName: "transform-move"
                label: "Master area position"
                enabled: page.ready
                themeLabel: "Use default"
                choices: ["left", "right", "top", "bottom", "center"]
                labels: ({ left: "Left", right: "Right", top: "Top", bottom: "Bottom", center: "Center" })
                value: page.master.orientation || null
                onChosen: v => page.setOption("master", "orientation", v)
                Layout.fillWidth: true
            }
        }
    }
    SettingsCard {
        title: "Scrolling"
        iconName: "object-columns"
        visible: scrollingGroup.shown

        OptionGroup {
            id: scrollingGroup
            settingId: "scrolling-options"
            source: page.sourceOf("scrolling-options")
            onReapply: lm.reapply("layouts")
            label: "Scrolling layout options"
            highlight: App.highlight
            enabled: page.ready
            inUse: page.inUse.indexOf("scrolling") >= 0
            Layout.fillWidth: true
            OverrideSlider {
                busy: lm.busy
                iconName: "object-columns"
                label: "Column width"
                enabled: page.ready
                from: 0.1
                to: 1
                fallback: page.num(page.dGroup("scrolling").column_width)
                value: page.saved(page.scrolling, "column_width")
                onChosen: v => page.setOption("scrolling", "column_width", v)
                Layout.fillWidth: true
            }
            OverrideSwitch {
                iconName: "view-fullscreen"
                label: "Fullscreen a single column"
                enabled: page.ready
                fallback: !!page.dGroup("scrolling").fullscreen_on_one_column
                value: page.saved(page.scrolling, "fullscreen_on_one_column")
                onChosen: v => page.setOption("scrolling", "fullscreen_on_one_column", v)
                Layout.fillWidth: true
            }
        }
    }
}
