// Look and layouts: gaps, borders, rounding, blur, shadows and animations, and how windows are
// tiled (the default layout, a layout per workspace, and each layout's options), on five tabs.
// The Windows tab draws a small screen with the gaps, borders and corners as the sliders set them. Everything is
// read from and written to uqd through LookModel; a refused change (including a config Hyprland
// rejects) shows the message and the controls go back to the last good value.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"

SettingsPage {
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
    /// Each layout drawn as window rectangles `[x, y, w, h]` (fractions of the tile).
    readonly property var layoutShapes: ({
        dwindle: [[0, 0, 0.5, 1], [0.5, 0, 0.5, 0.5], [0.5, 0.5, 0.25, 0.5], [0.75, 0.5, 0.25, 0.5]],
        master: [[0, 0, 0.55, 1], [0.55, 0, 0.45, 1 / 3], [0.55, 1 / 3, 0.45, 1 / 3], [0.55, 2 / 3, 0.45, 1 / 3]],
        scrolling: [[-0.3, 0, 0.42, 1], [0.12, 0, 0.6, 1], [0.72, 0, 0.5, 1]]
    })
    readonly property var layoutHelp: ({
        dwindle: "Each new window splits the space of the one before.",
        master: "One large window, the others stacked beside it.",
        scrolling: "Windows in columns on a strip that scrolls sideways."
    })
    /// The default layout in effect: the saved one, else uQ's default.
    readonly property string currentLayout: layouts.default || dLayouts.default || "dwindle"
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

    // A number in pixels (or steps) on a slider with its value beside it, or "Use theme" (null).
    // Like OverrideSpin, a change is sent 400 ms after the last move and the slider shows `value`
    // again once the model is done, so a refused change snaps back. `live` is what the slider
    // shows right now, for the preview.
    component PixelSlider: SettingRow {
        id: ps

        property var value: null
        property int from: 0
        property int to: 100
        property int fallback: 0
        property string unit: "px"
        property string useLabel: "Use theme"
        property bool busy: false
        readonly property int live: Math.round(sl.value)
        signal chosen(var value)

        highlight: App.highlight
        Layout.fillWidth: true

        QQC2.Slider {
            id: sl
            from: ps.from
            to: ps.to
            stepSize: 1
            snapMode: QQC2.Slider.SnapAlways
            value: ps.value === null ? ps.fallback : ps.value
            enabled: ps.enabled && ps.value !== null
            Accessible.name: ps.label
            Layout.preferredWidth: Kirigami.Units.gridUnit * 8
            onMoved: settle.restart()
        }
        UqLabel {
            objectName: "sliderValue"
            text: ps.live + (ps.unit !== "" ? " " + ps.unit : "")
            horizontalAlignment: Text.AlignRight
            opacity: sl.enabled ? 1 : 0.6
            Layout.minimumWidth: Kirigami.Units.gridUnit * 2.5
        }
        Timer {
            id: settle
            interval: 400
            onTriggered: ps.chosen(Math.round(sl.value))
        }
        onBusyChanged: {
            if (!busy)
                sl.value = Qt.binding(() => ps.value === null ? ps.fallback : ps.value);
        }
        QQC2.CheckBox {
            id: useBox
            checked: ps.value === null
            enabled: ps.enabled
            Accessible.name: ps.label + ": " + ps.useLabel.toLowerCase()
            onToggled: {
                settle.stop();
                ps.chosen(useBox.checked ? null : ps.fallback);
                useBox.checked = Qt.binding(() => ps.value === null);
            }
        }
        UqLabel {
            text: ps.useLabel
            MouseArea {
                anchors.fill: parent
                enabled: useBox.enabled
                onClicked: useBox.toggle()
            }
        }
    }

    // A layout drawn as its windows, small.
    component LayoutDrawing: Item {
        id: drawing
        property string layout: "dwindle"
        /// The windows as `[x, y, w, h]` fractions.
        property var shapes: []
        property bool active: false
        clip: true
        implicitWidth: Kirigami.Units.gridUnit * 5
        implicitHeight: Kirigami.Units.gridUnit * 3
        Accessible.ignored: true
        Repeater {
            model: drawing.shapes
            Rectangle {
                required property var modelData
                required property int index
                x: modelData[0] * drawing.width + 1
                y: modelData[1] * drawing.height + 1
                width: modelData[2] * drawing.width - 2
                height: modelData[3] * drawing.height - 2
                radius: Kirigami.Units.cornerRadius / 2
                color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.12)
                border.width: 1
                border.color: drawing.active && index === (drawing.layout === "scrolling" ? 1 : 0) ? Kirigami.Theme.highlightColor : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.3)
            }
        }
    }

    banners: [
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to change these settings."
            Layout.fillWidth: true
        },
        ErrorCard {
            objectName: "changeError"
            visible: lm.error !== ""
            title: "That change wasn't applied"
            detail: lm.error
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "Windows"
        iconName: "window"

        SettingsCard {
            title: "Gaps and borders"
            iconName: "distribute-horizontal-x"

            PixelSlider {
                id: gapsIn
                busy: lm.busy
                settingId: "gaps-inner"
                iconName: "view-split-left-right"
                source: page.sourceOf("gaps-inner")
                onReapply: lm.reapply("look")
                label: "Between windows"
                enabled: page.ready
                from: 0
                to: 50
                fallback: page.num(page.ui.gap_in)
                value: page.saved(page.look, "gaps_in")
                onChosen: v => page.setLook("gaps_in", v)
            }
            PixelSlider {
                id: gapsOut
                busy: lm.busy
                settingId: "gaps-outer"
                iconName: "view-fullscreen"
                source: page.sourceOf("gaps-outer")
                onReapply: lm.reapply("look")
                label: "Around the screen"
                enabled: page.ready
                from: 0
                to: 100
                fallback: page.num(page.ui.gap_out)
                value: page.saved(page.look, "gaps_out")
                onChosen: v => page.setLook("gaps_out", v)
            }
            PixelSlider {
                id: borderWidth
                busy: lm.busy
                settingId: "border-width"
                iconName: "format-border-style"
                source: page.sourceOf("border-width")
                onReapply: lm.reapply("look")
                label: "Border width"
                enabled: page.ready
                from: 0
                to: 20
                fallback: page.num(page.ui.border_width)
                value: page.saved(page.look, "border_width")
                onChosen: v => page.setLook("border_width", v)
            }
            PixelSlider {
                id: rounding
                busy: lm.busy
                settingId: "rounding"
                iconName: "draw-rectangle-rounded"
                source: page.sourceOf("rounding")
                onReapply: lm.reapply("look")
                label: "Corner rounding"
                enabled: page.ready
                from: 0
                to: 40
                fallback: page.num(page.ui.radius)
                value: page.saved(page.look, "rounding")
                onChosen: v => page.setLook("rounding", v)
            }
        }

        // A small screen with three windows, drawn at twice the scale of a 1920-wide screen so
        // a few pixels still show: what the four sliders do, as they move.
        SettingsCard {
            title: "Preview"
            iconName: "view-preview"

            Rectangle {
                id: screen
                objectName: "windowPreview"
                readonly property real scale: width / 960
                readonly property real gapIn: gapsIn.live * scale
                readonly property real gapOut: gapsOut.live * scale
                readonly property real bw: borderWidth.live > 0 ? Math.max(1, borderWidth.live * scale) : 0
                readonly property real rad: rounding.live * scale
                readonly property real innerW: width - gapOut * 2
                readonly property real innerH: height - gapOut * 2
                color: Qt.tint(Kirigami.Theme.backgroundColor, Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.25))
                radius: Kirigami.Units.cornerRadius
                clip: true
                Layout.fillWidth: true
                Layout.preferredHeight: width * 9 / 16
                Layout.maximumHeight: Kirigami.Units.gridUnit * 14
                Accessible.role: Accessible.Graphic
                Accessible.name: "Preview of the gaps, borders and corners"

                Repeater {
                    // The windows as fractions of the space inside the outer gap.
                    model: [[0, 0, 0.5, 1, true], [0.5, 0, 0.5, 0.5, false], [0.5, 0.5, 0.5, 0.5, false]]
                    Rectangle {
                        required property var modelData
                        x: screen.gapOut + modelData[0] * screen.innerW + (modelData[0] > 0 ? screen.gapIn / 2 : 0)
                        y: screen.gapOut + modelData[1] * screen.innerH + (modelData[1] > 0 ? screen.gapIn / 2 : 0)
                        width: modelData[2] * screen.innerW - (modelData[0] > 0 || modelData[2] < 1 ? screen.gapIn / 2 : 0)
                        height: modelData[3] * screen.innerH - (modelData[1] > 0 || modelData[3] < 1 ? screen.gapIn / 2 : 0)
                        radius: Math.min(screen.rad, width / 2, height / 2)
                        color: Kirigami.Theme.backgroundColor
                        border.width: screen.bw
                        border.color: modelData[4] ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
                        Behavior on x { NumberAnimation { duration: Kirigami.Units.shortDuration } }
                        Behavior on y { NumberAnimation { duration: Kirigami.Units.shortDuration } }
                        Behavior on width { NumberAnimation { duration: Kirigami.Units.shortDuration } }
                        Behavior on height { NumberAnimation { duration: Kirigami.Units.shortDuration } }

                        Column {
                            x: Math.max(parent.radius / 2, Kirigami.Units.smallSpacing) + parent.border.width
                            y: Math.max(parent.radius / 2, Kirigami.Units.smallSpacing) + parent.border.width
                            spacing: Kirigami.Units.smallSpacing
                            Repeater {
                                model: [0.6, 0.4, 0.5]
                                Rectangle {
                                    required property real modelData
                                    width: parent.parent.width * modelData
                                    height: Kirigami.Units.smallSpacing
                                    radius: height / 2
                                    color: Kirigami.Theme.disabledTextColor
                                    opacity: 0.5
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    SettingsTab {
        title: "Effects"
        iconName: "preferences-desktop-effects"

        SettingsCard {
            title: "Blur"
            iconName: "blur"

            OverrideSwitch {
                settingId: "blur"
                iconName: "blur"
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
            PixelSlider {
                busy: lm.busy
                settingId: "blur-size"
                iconName: "zoom-in"
                source: page.sourceOf("blur-size")
                onReapply: lm.reapply("look")
                label: "Blur size"
                help: "How far each pixel is spread."
                enabled: page.ready
                from: 1
                to: 20
                unit: ""
                useLabel: "Use default"
                fallback: page.num(page.dLook.blur_size)
                value: page.saved(page.look, "blur_size")
                onChosen: v => page.setLook("blur_size", v)
            }
            PixelSlider {
                busy: lm.busy
                settingId: "blur-passes"
                iconName: "media-playlist-repeat"
                source: page.sourceOf("blur-passes")
                onReapply: lm.reapply("look")
                label: "Blur passes"
                help: "More passes look smoother and use more power."
                enabled: page.ready
                from: 1
                to: 4
                unit: ""
                useLabel: "Use default"
                fallback: page.num(page.dLook.blur_passes)
                value: page.saved(page.look, "blur_passes")
                onChosen: v => page.setLook("blur_passes", v)
            }
        }
        SettingsCard {
            title: "Shadows"
            iconName: "object-order-front"

            OverrideSwitch {
                settingId: "shadow"
                iconName: "object-order-front"
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
        }
    }

    SettingsTab {
        title: "Motion"
        iconName: "tool-animator"

        SettingsCard {
            title: "Animations"
            iconName: "tool-animator"

            OverrideSwitch {
                settingId: "animations"
                iconName: "media-playback-start-symbolic"
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
                iconName: "speedometer"
                source: page.sourceOf("animation-speed")
                onReapply: lm.reapply("look")
                label: "Speed"
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
        }
        SettingsCard {
            title: "Comfort"
            iconName: "preferences-desktop-accessibility"

            SwitchRow {
                objectName: "reduceMotion"
                settingId: "reduce-motion"
                iconName: "preferences-desktop-accessibility"
                label: "Reduce motion"
                help: "Turns window animations off; fades and wallpaper changes happen at once."
                highlight: App.highlight
                value: !!page.snap.reduce_motion
                enabled: page.ready && !lm.busy
                Layout.fillWidth: true
                onChosen: on => lm.setReduceMotion(on)
            }
        }
    }

    SettingsTab {
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

    SettingsTab {
        title: "Workspaces"
        iconName: "virtual-desktops"

        SettingsCard {
            title: "Workspaces"
            iconName: "virtual-desktops"

            WorkspaceRules {
                highlight: App.highlight
                enabled: page.ready
                workspaces: page.workspaces
                layoutNames: page.layoutNames
                layoutLabels: page.layoutLabels
                onSetRule: (selector, layout) => lm.setWorkspace(selector, layout)
                Layout.fillWidth: true
            }
        }
        // Workspace settings that live on other pages, one click away.
        SettingsCard {
            title: "Elsewhere"
            iconName: "go-next-view"

            SettingRow {
                objectName: "toDisplays"
                iconName: "preferences-desktop-display"
                label: "Which display each workspace is on"
                Layout.fillWidth: true
                QQC2.Button {
                    text: "Displays"
                    icon.name: "go-next"
                    Accessible.name: "Open Displays"
                    onClicked: App.showPage("displays", "")
                }
            }
            SettingRow {
                objectName: "toRules"
                iconName: "preferences-system-windows-actions"
                label: "Apps that open on a workspace"
                Layout.fillWidth: true
                QQC2.Button {
                    text: "Window rules"
                    icon.name: "go-next"
                    Accessible.name: "Open Window rules"
                    onClicked: App.showPage("rules", "")
                }
            }
        }
    }
}
