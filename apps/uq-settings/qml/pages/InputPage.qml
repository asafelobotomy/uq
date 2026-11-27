// Input: keyboard layouts and the keys that switch between them, key repeat, the mouse, the
// touchpad and its gestures, and settings for one pointing device. Everything is read from and
// written to the settings service through InputModel; a refused change shows why and the control
// goes back.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"
import "input"

SettingsPage {
    id: page

    readonly property var snap: {
        try {
            return JSON.parse(im.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property bool ready: App.uqdUp
    readonly property var input: snap.input || {}
    readonly property var keyboard: input.keyboard || {}
    readonly property var mouse: input.mouse || {}
    readonly property var touchpad: input.touchpad || {}
    readonly property var deviceSettings: input.devices || {}
    readonly property var devices: snap.devices || {}
    readonly property var pointers: devices.pointers || []
    readonly property var xkb: snap.xkb || {}
    readonly property var defaults: snap.defaults || {}
    readonly property var dKeyboard: defaults.keyboard || {}
    readonly property var dTouchpad: defaults.touchpad || {}
    readonly property var dSwipe: defaults.swipe || {}
    readonly property var swipe: snap.swipe || {}
    readonly property var sources: snap.sources || ({})
    readonly property var switchNames: {
        const m = {};
        for (const o of (xkb.switch_options || []))
            m[o[0]] = o[1];
        return m;
    }
    readonly property bool hasTouchpad: pointers.some(p => String(p.name).indexOf("touchpad") >= 0) || Object.keys(touchpad).length > 0
    readonly property var accelNames: ["Default", "Adaptive", "Flat"]
    readonly property var accelCodes: ["default", "adaptive", "flat"]
    /// The pointing devices that don't have their own settings yet.
    readonly property var addable: page.pointers.filter(p => page.deviceSettings[p.name] === undefined)

    function saved(obj, name) {
        return obj[name] === undefined ? null : obj[name];
    }
    function setKeyboard(name, value) {
        im.setKeyboard(name, JSON.stringify(value));
    }
    function setMouse(name, value) {
        im.setMouse(name, JSON.stringify(value));
    }
    function setTouchpad(name, value) {
        im.setTouchpad(name, JSON.stringify(value));
    }
    function setSwipe(name, value) {
        im.setSwipe(name, JSON.stringify(value));
    }
    /// Where a row's live value comes from (`{source, detail}`), or null: see SettingRow.
    function sourceOf(settingId) {
        return page.sources[settingId] ?? null;
    }
    function setDevice(device, name, value) {
        im.setDevice(device, name, JSON.stringify(value));
    }
    function layoutsWithout(i) {
        const l = (keyboard.layouts || []).slice();
        l.splice(i, 1);
        return l;
    }
    /// A speed of -1 to 1 as people read it: "+0.25", "0", "-0.5".
    function signed(v) {
        const r = Math.round(v * 100) / 100;
        return r > 0 ? "+" + r : String(r);
    }
    function refresh() {
        im.refresh();
    }

    Component.onCompleted: im.refresh()

    InputModel {
        id: im
    }

    GestureEditor {
        id: gestureEditor
        parent: QQC2.Overlay.overlay
        choices: page.snap.choices || ({})
        list: page.snap.gestures || []
        check: (index, json) => im.draftClash(index, json)
        canSave: page.ready && !im.busy
        busy: im.busy
        errorNow: () => im.error
        onGestureAccepted: (index, json) => {
            im.saveGesture(index, json);
            // No request went out (no session bus): say so here.
            if (!im.busy)
                gestureEditor.finishSave(im.error !== "" ? im.error : "uQ isn't running");
        }
    }

    Connections {
        target: App
        function onUqdUpChanged() {
            if (App.uqdUp)
                im.refresh();
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
            visible: im.error !== ""
            title: "That change wasn't applied"
            detail: im.error
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "Keyboard"
        iconName: "input-keyboard"

        SettingsCard {
            title: "Layouts"
            iconName: "keyboard-layout"

            SettingRow {
                settingId: "keyboard-layouts"
                iconName: "favorite"
                label: "The first one is the one you start with"
                highlight: App.highlight
                enabled: page.ready
                source: page.sourceOf("keyboard-layouts")
                Layout.fillWidth: true
            }
            KeyboardLayoutList {
                layouts: page.keyboard.layouts || []
                xkbLayouts: page.xkb.layouts || []
                variants: page.xkb.variants || ({})
                enabled: page.ready && !im.busy
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                onReordered: l => page.setKeyboard("layouts", l)
                onRemoved: i => page.setKeyboard("layouts", page.layoutsWithout(i))
                onAdded: (code, variant) => {
                    const entry = variant ? { layout: code, variant: variant } : { layout: code };
                    page.setKeyboard("layouts", (page.keyboard.layouts || []).concat([entry]));
                }
            }
            OverrideCombo {
                settingId: "layout-switch"
                iconName: "document-swap"
                label: "Switch layouts with"
                help: "Needs two layouts or more."
                highlight: App.highlight
                enabled: page.ready && (page.keyboard.layouts || []).length > 1
                source: page.sourceOf("layout-switch")
                themeLabel: "Don't switch with keys"
                choices: (page.xkb.switch_options || []).map(o => o[0])
                labels: page.switchNames
                value: page.saved(page.keyboard, "switch_combo")
                onChosen: v => page.setKeyboard("switch_combo", v)
                Layout.fillWidth: true
            }
        }

        SettingsCard {
            title: "Holding a key down"
            iconName: "media-repeat-all"

            OverrideSpin {
                busy: im.busy
                settingId: "repeat-delay"
                iconName: "chronometer"
                label: "Delay before keys repeat"
                highlight: App.highlight
                enabled: page.ready
                source: page.sourceOf("repeat-delay")
                useLabel: "Use default"
                from: 100
                to: 2000
                fallback: page.dKeyboard.repeat_delay || 600
                unit: "ms"
                value: page.saved(page.keyboard, "repeat_delay")
                onChosen: v => page.setKeyboard("repeat_delay", v)
                Layout.fillWidth: true
            }
            OverrideSpin {
                busy: im.busy
                settingId: "repeat-rate"
                iconName: "media-seek-forward"
                label: "Key repeat speed"
                highlight: App.highlight
                enabled: page.ready
                source: page.sourceOf("repeat-rate")
                useLabel: "Use default"
                from: 1
                to: 100
                fallback: page.dKeyboard.repeat_rate || 25
                unit: "per second"
                value: page.saved(page.keyboard, "repeat_rate")
                onChosen: v => page.setKeyboard("repeat_rate", v)
                Layout.fillWidth: true
            }
            QQC2.TextField {
                placeholderText: "Hold a key here to try it"
                Accessible.name: "Hold a key here to try it"
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
            }
        }
    }

    MouseTab {
        im: im
        page: page
    }

    SettingsTab {
        title: "Touchpad"
        iconName: "input-touchpad"
        available: page.hasTouchpad

        SettingsCard {
            title: "Tapping and typing"
            iconName: "input-touchpad"

            OverrideSwitch {
                settingId: "tap-to-click"
                iconName: "hand"
                label: "Tap to click"
                highlight: App.highlight
                enabled: page.ready
                source: page.sourceOf("tap-to-click")
                fallback: page.dTouchpad.tap_to_click !== false
                value: page.saved(page.touchpad, "tap_to_click")
                onChosen: v => page.setTouchpad("tap_to_click", v)
                Layout.fillWidth: true
            }
            OverrideSwitch {
                settingId: "disable-while-typing"
                iconName: "input-keyboard"
                label: "Turn the touchpad off while typing"
                highlight: App.highlight
                enabled: page.ready
                source: page.sourceOf("disable-while-typing")
                fallback: page.dTouchpad.disable_while_typing !== false
                value: page.saved(page.touchpad, "disable_while_typing")
                onChosen: v => page.setTouchpad("disable_while_typing", v)
                Layout.fillWidth: true
            }
        }

        SettingsCard {
            title: "Scrolling"
            iconName: "transform-browse"

            OverrideSwitch {
                settingId: "touchpad-natural-scroll"
                iconName: "transform-browse"
                label: "Natural scrolling on the touchpad"
                help: "The page follows your fingers, as on a phone."
                highlight: App.highlight
                enabled: page.ready
                source: page.sourceOf("touchpad-natural-scroll")
                fallback: page.dTouchpad.natural_scroll === true
                value: page.saved(page.touchpad, "natural_scroll")
                onChosen: v => page.setTouchpad("natural_scroll", v)
                Layout.fillWidth: true
            }
            OverrideSlider {
                busy: im.busy
                settingId: "touchpad-scroll-speed"
                iconName: "arrow-down"
                label: "Touchpad scroll speed"
                highlight: App.highlight
                enabled: page.ready
                source: page.sourceOf("touchpad-scroll-speed")
                from: 0.1
                to: 5
                stepSize: 0.1
                fallback: page.dTouchpad.scroll_factor || 1
                lowLabel: "Slower"
                highLabel: "Faster"
                value: page.saved(page.touchpad, "scroll_factor")
                onChosen: v => page.setTouchpad("scroll_factor", v)
                Layout.fillWidth: true
            }
        }
    }

    SettingsTab {
        title: "Gestures"
        iconName: "preferences-desktop-gestures-touch"
        available: page.hasTouchpad

        SettingsCard {
            title: "Touchpad gestures"
            iconName: "preferences-desktop-gestures-touch"

            SettingRow {
                settingId: "gestures"
                iconName: "hand"
                label: "Your gestures"
                highlight: App.highlight
                enabled: page.ready
                Layout.fillWidth: true
            }
            GestureList {
                gestures: page.snap.gestures || []
                defaults: page.snap.gestures_default !== false
                clashes: page.snap.clashes || ({})
                userLua: page.snap.user_lua || null
                idle: page.ready && !im.busy
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                onToggled: (index, enabled) => im.toggleGesture(index, enabled)
                onRemoved: index => im.removeGesture(index)
                onChangeRequested: index => {
                    const row = (page.snap.gestures || [])[index];
                    if (row)
                        gestureEditor.change(index, row.gesture);
                }
                onAddRequested: gestureEditor.startNew()
                onResetRequested: im.resetGestures()
            }
        }

        SettingsCard {
            title: "Swiping between workspaces"
            iconName: "virtual-desktops"

            OverrideSpin {
                busy: im.busy
                settingId: "gesture-swipe-distance"
                iconName: "transform-move"
                label: "Swipe distance"
                help: "How far a full swipe is, in touchpad units."
                highlight: App.highlight
                enabled: page.ready
                source: page.sourceOf("gesture-swipe-distance")
                useLabel: "Use default"
                from: 0
                to: 2000
                fallback: page.dSwipe.distance || 300
                value: page.saved(page.swipe, "distance")
                onChosen: v => page.setSwipe("distance", v)
                Layout.fillWidth: true
            }
            OverrideSlider {
                busy: im.busy
                settingId: "gesture-swipe-cancel"
                iconName: "go-next"
                label: "How far to swipe before it switches"
                highlight: App.highlight
                enabled: page.ready
                source: page.sourceOf("gesture-swipe-cancel")
                from: 0
                to: 1
                stepSize: 0.05
                fallback: page.dSwipe.cancel_ratio ?? 0.5
                lowLabel: "A little"
                highLabel: "Most"
                value: page.saved(page.swipe, "cancel_ratio")
                onChosen: v => page.setSwipe("cancel_ratio", v)
                Layout.fillWidth: true
            }
            OverrideSwitch {
                settingId: "gesture-swipe-invert"
                iconName: "document-swap"
                label: "Reverse swipe direction"
                highlight: App.highlight
                enabled: page.ready
                source: page.sourceOf("gesture-swipe-invert")
                fallback: page.dSwipe.invert !== false
                value: page.saved(page.swipe, "invert")
                onChosen: v => page.setSwipe("invert", v)
                Layout.fillWidth: true
            }
            OverrideSwitch {
                settingId: "gesture-swipe-create"
                iconName: "list-add"
                label: "Swipe past the last workspace to make a new one"
                highlight: App.highlight
                enabled: page.ready
                source: page.sourceOf("gesture-swipe-create")
                fallback: page.dSwipe.create_new !== false
                value: page.saved(page.swipe, "create_new")
                onChosen: v => page.setSwipe("create_new", v)
                Layout.fillWidth: true
            }
        }
    }

    DevicesTab {
        im: im
        page: page
    }
}
