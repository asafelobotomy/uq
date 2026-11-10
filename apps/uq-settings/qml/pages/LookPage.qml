// Look and layouts: gaps, borders, rounding, blur, shadows and animations, and how windows are
// tiled (the default layout, a layout per workspace, and each layout's options). Everything is
// read from and written to uqd through LookModel; a refused change (including a config Hyprland
// rejects) shows the message and the controls go back to the last good value.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"

QQC2.ScrollView {
    id: page

    readonly property var snap: {
        try {
            return JSON.parse(lm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var look: snap.look || {}
    readonly property var layouts: snap.layouts || {}
    readonly property var workspaces: layouts.workspaces || {}
    readonly property var dwindle: layouts.dwindle || {}
    readonly property var master: layouts.master || {}
    readonly property var scrolling: layouts.scrolling || {}
    readonly property bool ready: App.uqdUp
    readonly property var layoutNames: ["dwindle", "master", "scrolling"]
    /// The rendered theme's tokens: what "Use theme" means right now, for seeding overrides.
    readonly property var tokens: {
        try {
            return JSON.parse(App.themeJson || "{}");
        } catch (e) {
            return {};
        }
    }
    readonly property var ui: tokens.ui || {}
    /// The layouts in use, so a layout's options show only when something uses them
    /// (Hyprland's own default is dwindle).
    readonly property var inUse: {
        const used = [layouts.default || "dwindle"];
        for (const k of Object.keys(workspaces))
            used.push(workspaces[k]);
        return used;
    }
    readonly property var ruleNames: Object.keys(workspaces).sort()

    /// A saved value, or null when the section has none (or no value for it).
    function saved(obj, name) {
        return obj[name] === undefined ? null : obj[name];
    }
    function setLook(name, value) {
        lm.setLook(name, JSON.stringify(value));
    }
    function setOption(group, name, value) {
        lm.setLayoutOption(group, name, JSON.stringify(value));
    }

    Component.onCompleted: lm.refresh()
    Accessible.role: Accessible.Pane
    Accessible.name: "Look and layouts"
    contentWidth: availableWidth

    LookModel {
        id: lm
    }

    Connections {
        target: App
        function onUqdUpChanged() {
            if (App.uqdUp)
                lm.refresh();
        }
    }

    ColumnLayout {
        id: column
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 40)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Look and layouts"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start uqd to change these settings."
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "changeError"
            visible: lm.error !== ""
            title: "That change wasn't applied"
            detail: lm.error
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Windows"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        OverrideSpin {
            busy: lm.busy
            settingId: "gaps-inner"
            label: "Gaps between windows"
            highlight: App.highlight
            enabled: page.ready
            from: 0
            to: 50
            fallback: page.ui.gap_in !== undefined ? page.ui.gap_in : 5
            unit: "px"
            value: page.saved(page.look, "gaps_in")
            onChosen: v => page.setLook("gaps_in", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: lm.busy
            settingId: "gaps-outer"
            label: "Gaps around the screen"
            highlight: App.highlight
            enabled: page.ready
            from: 0
            to: 100
            fallback: page.ui.gap_out !== undefined ? page.ui.gap_out : 10
            unit: "px"
            value: page.saved(page.look, "gaps_out")
            onChosen: v => page.setLook("gaps_out", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: lm.busy
            settingId: "border-width"
            label: "Window border width"
            highlight: App.highlight
            enabled: page.ready
            from: 0
            to: 20
            fallback: page.ui.border_width !== undefined ? page.ui.border_width : 2
            unit: "px"
            value: page.saved(page.look, "border_width")
            onChosen: v => page.setLook("border_width", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: lm.busy
            settingId: "rounding"
            label: "Corner rounding"
            highlight: App.highlight
            enabled: page.ready
            from: 0
            to: 40
            fallback: page.ui.radius !== undefined ? page.ui.radius : 10
            unit: "px"
            value: page.saved(page.look, "rounding")
            onChosen: v => page.setLook("rounding", v)
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Effects"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        OverrideSwitch {
            settingId: "blur"
            label: "Blur behind windows"
            highlight: App.highlight
            enabled: page.ready
            fallback: true
            value: page.saved(page.look, "blur")
            onChosen: v => page.setLook("blur", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: lm.busy
            settingId: "blur-size"
            label: "Blur size"
            highlight: App.highlight
            enabled: page.ready
            from: 1
            to: 20
            fallback: 8
            useLabel: "Use default"
            value: page.saved(page.look, "blur_size")
            onChosen: v => page.setLook("blur_size", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: lm.busy
            settingId: "blur-passes"
            label: "Blur passes"
            highlight: App.highlight
            enabled: page.ready
            from: 1
            to: 4
            fallback: 1
            useLabel: "Use default"
            value: page.saved(page.look, "blur_passes")
            onChosen: v => page.setLook("blur_passes", v)
            Layout.fillWidth: true
        }
        OverrideSwitch {
            settingId: "shadow"
            label: "Window shadows"
            highlight: App.highlight
            enabled: page.ready
            fallback: true
            value: page.saved(page.look, "shadow")
            onChosen: v => page.setLook("shadow", v)
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Motion"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        OverrideSwitch {
            settingId: "animations"
            label: "Animations"
            highlight: App.highlight
            enabled: page.ready
            fallback: true
            value: page.saved(page.look, "animations")
            onChosen: v => page.setLook("animations", v)
            Layout.fillWidth: true
        }
        OverrideSlider {
            busy: lm.busy
            settingId: "animation-speed"
            label: "Animation speed"
            highlight: App.highlight
            enabled: page.ready
            from: 0.25
            to: 4
            stepSize: 0.05
            fallback: 1
            lowLabel: "Slower"
            highLabel: "Faster"
            value: page.saved(page.look, "animation_speed")
            onChosen: v => page.setLook("animation_speed", v)
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Layouts"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        OverrideCombo {
            settingId: "default-layout"
            label: "Default layout"
            highlight: App.highlight
            enabled: page.ready
            themeLabel: "Use default"
            choices: page.layoutNames
            editable: false
            value: page.layouts.default || null
            onChosen: v => page.setOption("", "default", v)
            Layout.fillWidth: true
        }
        SettingRow {
            settingId: "workspace-layouts"
            label: "Layout for each workspace"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
            UqLabel {
                text: "No workspace has its own layout."
                visible: page.ruleNames.length === 0
            }
        }
        Repeater {
            model: page.ruleNames
            SettingRow {
                id: rule
                required property string modelData
                readonly property string layout: page.workspaces[modelData] || ""
                objectName: "rule"
                label: "Workspace " + modelData
                enabled: page.ready
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing * 2
                UqComboBox {
                    model: page.layoutNames
                    currentIndex: page.layoutNames.indexOf(rule.layout)
                    Accessible.name: "Layout for workspace " + rule.modelData
                    onActivated: index => {
                        lm.setWorkspace(rule.modelData, page.layoutNames[index]);
                        currentIndex = Qt.binding(() => page.layoutNames.indexOf(rule.layout));
                    }
                }
                QQC2.Button {
                    text: "Remove"
                    Accessible.name: "Remove the rule for workspace " + rule.modelData
                    onClicked: lm.setWorkspace(rule.modelData, "")
                }
            }
        }
        SettingRow {
            id: addRule
            // The draft is cleared only when the saved rules change, so a refused add keeps it.
            readonly property string savedRules: JSON.stringify(page.workspaces)
            objectName: "addRule"
            label: "Add a workspace rule"
            enabled: page.ready
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
            onSavedRulesChanged: selector.text = ""
            QQC2.TextField {
                id: selector
                placeholderText: "Workspace, such as 3 or special:scratch"
                Accessible.name: "Workspace to add a rule for"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 14
            }
            UqComboBox {
                id: newLayout
                model: page.layoutNames
                Accessible.name: "Layout for the new rule"
            }
            QQC2.Button {
                text: "Add"
                enabled: selector.text.trim() !== ""
                Accessible.name: "Add the workspace rule"
                onClicked: lm.setWorkspace(selector.text.trim(), page.layoutNames[newLayout.currentIndex])
            }
        }

        // Each layout's options, shown only when the default or a workspace rule uses it.
        SettingRow {
            settingId: "dwindle-options"
            label: "Dwindle layout options"
            highlight: App.highlight
            enabled: page.ready
            visible: page.inUse.indexOf("dwindle") >= 0
            Layout.fillWidth: true
        }
        OverrideSwitch {
            label: "Keep the split direction"
            visible: page.inUse.indexOf("dwindle") >= 0
            enabled: page.ready
            fallback: false
            value: page.saved(page.dwindle, "preserve_split")
            onChosen: v => page.setOption("dwindle", "preserve_split", v)
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        }
        OverrideSwitch {
            label: "Smart split"
            visible: page.inUse.indexOf("dwindle") >= 0
            enabled: page.ready
            fallback: false
            value: page.saved(page.dwindle, "smart_split")
            onChosen: v => page.setOption("dwindle", "smart_split", v)
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        }
        OverrideCombo {
            label: "Where new windows go"
            visible: page.inUse.indexOf("dwindle") >= 0
            enabled: page.ready
            themeLabel: "Use default"
            choices: ["follow_mouse", "left", "right"]
            value: page.dwindle.force_split || null
            onChosen: v => page.setOption("dwindle", "force_split", v)
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        }

        SettingRow {
            settingId: "master-options"
            label: "Master layout options"
            highlight: App.highlight
            enabled: page.ready
            visible: page.inUse.indexOf("master") >= 0
            Layout.fillWidth: true
        }
        OverrideSlider {
            busy: lm.busy
            label: "Master area share"
            visible: page.inUse.indexOf("master") >= 0
            enabled: page.ready
            from: 0.1
            to: 0.9
            fallback: 0.55
            value: page.saved(page.master, "mfact")
            onChosen: v => page.setOption("master", "mfact", v)
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        }
        OverrideCombo {
            label: "New windows join as"
            visible: page.inUse.indexOf("master") >= 0
            enabled: page.ready
            themeLabel: "Use default"
            choices: ["master", "slave", "inherit"]
            value: page.master.new_status || null
            onChosen: v => page.setOption("master", "new_status", v)
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        }
        OverrideCombo {
            label: "Master area position"
            visible: page.inUse.indexOf("master") >= 0
            enabled: page.ready
            themeLabel: "Use default"
            choices: ["left", "right", "top", "bottom", "center"]
            value: page.master.orientation || null
            onChosen: v => page.setOption("master", "orientation", v)
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        }

        SettingRow {
            settingId: "scrolling-options"
            label: "Scrolling layout options"
            highlight: App.highlight
            enabled: page.ready
            visible: page.inUse.indexOf("scrolling") >= 0
            Layout.fillWidth: true
        }
        OverrideSlider {
            busy: lm.busy
            label: "Column width"
            visible: page.inUse.indexOf("scrolling") >= 0
            enabled: page.ready
            from: 0.1
            to: 1
            fallback: 0.5
            value: page.saved(page.scrolling, "column_width")
            onChosen: v => page.setOption("scrolling", "column_width", v)
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        }
        OverrideSwitch {
            label: "Fullscreen a single column"
            visible: page.inUse.indexOf("scrolling") >= 0
            enabled: page.ready
            fallback: false
            value: page.saved(page.scrolling, "fullscreen_on_one_column")
            onChosen: v => page.setOption("scrolling", "fullscreen_on_one_column", v)
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
