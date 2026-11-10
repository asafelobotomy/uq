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
    /// What "Use default" means for each field, from uqd's page state (one place, in Rust).
    readonly property var defaults: snap.defaults || {}
    readonly property var dLook: defaults.look || {}
    readonly property var dLayouts: defaults.layouts || {}
    /// Where each row's live value comes from, by setting id (uqd's Explain); {} when unknown.
    readonly property var sources: snap.sources || {}
    readonly property var layoutLabels: ({ dwindle: "Dwindle", master: "Master", scrolling: "Scrolling" })
    readonly property var splitLabels: ({ follow_mouse: "Follow the mouse", left: "Always left", right: "Always right" })
    /// The layouts in use, so a layout's options show only when something uses them
    /// (Hyprland's own default is dwindle).
    readonly property var inUse: {
        const used = [layouts.default || "dwindle"];
        for (const k of Object.keys(workspaces))
            used.push(workspaces[k]);
        return used;
    }

    /// A saved value, or null when the section has none (or no value for it).
    function saved(obj, name) {
        return obj[name] === undefined ? null : obj[name];
    }
    function num(v) {
        return v === undefined ? 0 : v;
    }
    function dGroup(name) {
        return dLayouts[name] || ({});
    }
    function setLook(name, value) {
        lm.setLook(name, JSON.stringify(value));
    }
    function setOption(group, name, value) {
        lm.setLayoutOption(group, name, JSON.stringify(value));
    }
    /// Where a row's live value comes from (`{source, detail}`), or null: see SettingRow.
    function sourceOf(settingId) {
        return page.sources[settingId] ?? null;
    }
    /// Read everything again; Main calls it when the window comes back to the front.
    function refresh() {
        lm.refresh();
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
            detail: "Start the uQ desktop service to change these settings."
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
            source: page.sourceOf("gaps-inner")
            onReapply: lm.reapply("look")
            label: "Gaps between windows"
            highlight: App.highlight
            enabled: page.ready
            from: 0
            to: 50
            fallback: page.num(page.ui.gap_in)
            unit: "px"
            value: page.saved(page.look, "gaps_in")
            onChosen: v => page.setLook("gaps_in", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: lm.busy
            settingId: "gaps-outer"
            source: page.sourceOf("gaps-outer")
            onReapply: lm.reapply("look")
            label: "Gaps around the screen"
            highlight: App.highlight
            enabled: page.ready
            from: 0
            to: 100
            fallback: page.num(page.ui.gap_out)
            unit: "px"
            value: page.saved(page.look, "gaps_out")
            onChosen: v => page.setLook("gaps_out", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: lm.busy
            settingId: "border-width"
            source: page.sourceOf("border-width")
            onReapply: lm.reapply("look")
            label: "Window border width"
            highlight: App.highlight
            enabled: page.ready
            from: 0
            to: 20
            fallback: page.num(page.ui.border_width)
            unit: "px"
            value: page.saved(page.look, "border_width")
            onChosen: v => page.setLook("border_width", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: lm.busy
            settingId: "rounding"
            source: page.sourceOf("rounding")
            onReapply: lm.reapply("look")
            label: "Corner rounding"
            highlight: App.highlight
            enabled: page.ready
            from: 0
            to: 40
            fallback: page.num(page.ui.radius)
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
            source: page.sourceOf("blur")
            onReapply: lm.reapply("look")
            label: "Blur behind windows"
            highlight: App.highlight
            enabled: page.ready
            fallback: !!page.dLook.blur
            value: page.saved(page.look, "blur")
            onChosen: v => page.setLook("blur", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: lm.busy
            settingId: "blur-size"
            source: page.sourceOf("blur-size")
            onReapply: lm.reapply("look")
            label: "Blur size"
            highlight: App.highlight
            enabled: page.ready
            from: 1
            to: 20
            fallback: page.num(page.dLook.blur_size)
            useLabel: "Use default"
            value: page.saved(page.look, "blur_size")
            onChosen: v => page.setLook("blur_size", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: lm.busy
            settingId: "blur-passes"
            source: page.sourceOf("blur-passes")
            onReapply: lm.reapply("look")
            label: "Blur passes"
            highlight: App.highlight
            enabled: page.ready
            from: 1
            to: 4
            fallback: page.num(page.dLook.blur_passes)
            useLabel: "Use default"
            value: page.saved(page.look, "blur_passes")
            onChosen: v => page.setLook("blur_passes", v)
            Layout.fillWidth: true
        }
        OverrideSwitch {
            settingId: "shadow"
            source: page.sourceOf("shadow")
            onReapply: lm.reapply("look")
            label: "Window shadows"
            highlight: App.highlight
            enabled: page.ready
            fallback: !!page.dLook.shadow
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
            source: page.sourceOf("animations")
            onReapply: lm.reapply("look")
            label: "Animations"
            highlight: App.highlight
            enabled: page.ready
            fallback: !!page.dLook.animations
            value: page.saved(page.look, "animations")
            onChosen: v => page.setLook("animations", v)
            Layout.fillWidth: true
        }
        OverrideSlider {
            busy: lm.busy
            settingId: "animation-speed"
            source: page.sourceOf("animation-speed")
            onReapply: lm.reapply("look")
            label: "Animation speed"
            highlight: App.highlight
            enabled: page.ready
            from: 0.25
            to: 4
            stepSize: 0.05
            fallback: page.num(page.dLook.animation_speed)
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
            source: page.sourceOf("default-layout")
            onReapply: lm.reapply("layouts")
            label: "Default layout"
            highlight: App.highlight
            enabled: page.ready
            themeLabel: "Use default"
            choices: page.layoutNames
            labels: page.layoutLabels
            editable: false
            value: page.layouts.default || null
            onChosen: v => page.setOption("", "default", v)
            Layout.fillWidth: true
        }
        WorkspaceRules {
            highlight: App.highlight
            enabled: page.ready
            workspaces: page.workspaces
            layoutNames: page.layoutNames
            layoutLabels: page.layoutLabels
            onSetRule: (selector, layout) => lm.setWorkspace(selector, layout)
            Layout.fillWidth: true
        }

        // Each layout's options, shown when the default or a workspace rule uses the layout (or
        // a search hit points at them).
        OptionGroup {
            settingId: "dwindle-options"
            source: page.sourceOf("dwindle-options")
            onReapply: lm.reapply("layouts")
            label: "Dwindle layout options"
            highlight: App.highlight
            enabled: page.ready
            inUse: page.inUse.indexOf("dwindle") >= 0
            Layout.fillWidth: true
            OverrideSwitch {
                label: "Keep the split direction"
                enabled: page.ready
                fallback: !!page.dGroup("dwindle").preserve_split
                value: page.saved(page.dwindle, "preserve_split")
                onChosen: v => page.setOption("dwindle", "preserve_split", v)
                Layout.fillWidth: true
            }
            OverrideSwitch {
                label: "Smart split"
                enabled: page.ready
                fallback: !!page.dGroup("dwindle").smart_split
                value: page.saved(page.dwindle, "smart_split")
                onChosen: v => page.setOption("dwindle", "smart_split", v)
                Layout.fillWidth: true
            }
            OverrideCombo {
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
        OptionGroup {
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
        OptionGroup {
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
                label: "Fullscreen a single column"
                enabled: page.ready
                fallback: !!page.dGroup("scrolling").fullscreen_on_one_column
                value: page.saved(page.scrolling, "fullscreen_on_one_column")
                onChosen: v => page.setOption("scrolling", "fullscreen_on_one_column", v)
                Layout.fillWidth: true
            }
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
