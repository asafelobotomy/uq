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
import "look"

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

    WindowLookTab {
        lm: lm
        page: page
    }

    EffectsTab {
        lm: lm
        page: page
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

    LayoutsTab {
        lm: lm
        page: page
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
