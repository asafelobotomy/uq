// Input: keyboard layouts and the keys that switch between them, key repeat, the mouse, the
// touchpad, and settings for one pointing device. Everything is read from and written to the
// settings service through InputModel; a refused change shows why and the control goes back.
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
    readonly property var switchNames: {
        const m = {};
        for (const o of (xkb.switch_options || []))
            m[o[0]] = o[1];
        return m;
    }
    readonly property bool hasTouchpad: pointers.some(p => String(p.name).indexOf("touchpad") >= 0) || Object.keys(touchpad).length > 0
    readonly property var accelNames: ["Default", "Adaptive", "Flat"]
    readonly property var accelCodes: ["default", "adaptive", "flat"]
    property bool showDeviceNames: false

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
    function setDevice(device, name, value) {
        im.setDevice(device, name, JSON.stringify(value));
    }
    function layoutsWithout(i) {
        const l = (keyboard.layouts || []).slice();
        l.splice(i, 1);
        return l;
    }
    function refresh() {
        im.refresh();
    }

    Component.onCompleted: im.refresh()
    Accessible.role: Accessible.Pane
    Accessible.name: "Input"
    contentWidth: availableWidth

    InputModel {
        id: im
    }

    Connections {
        target: App
        function onUqdUpChanged() {
            if (App.uqdUp)
                im.refresh();
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
            text: "Input"
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
            visible: im.error !== ""
            title: "That change wasn't applied"
            detail: im.error
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Keyboard"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SettingRow {
            settingId: "keyboard-layouts"
            label: "Keyboard layouts"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            KeyboardLayoutList {
                layouts: page.keyboard.layouts || []
                xkbLayouts: page.xkb.layouts || []
                variants: page.xkb.variants || ({})
                enabled: page.ready && !im.busy
                Layout.fillWidth: true
                onReordered: l => page.setKeyboard("layouts", l)
                onRemoved: i => page.setKeyboard("layouts", page.layoutsWithout(i))
                onAdded: (code, variant) => {
                    const entry = variant ? { layout: code, variant: variant } : { layout: code };
                    page.setKeyboard("layouts", (page.keyboard.layouts || []).concat([entry]));
                }
            }
        }
        OverrideCombo {
            settingId: "layout-switch"
            label: "Switch layouts with"
            highlight: App.highlight
            enabled: page.ready && (page.keyboard.layouts || []).length > 1
            themeLabel: "Don't switch with keys"
            choices: (page.xkb.switch_options || []).map(o => o[0])
            labels: page.switchNames
            value: page.saved(page.keyboard, "switch_combo")
            onChosen: v => page.setKeyboard("switch_combo", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: im.busy
            settingId: "repeat-delay"
            label: "Delay before keys repeat"
            highlight: App.highlight
            enabled: page.ready
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
            label: "Repeat speed"
            highlight: App.highlight
            enabled: page.ready
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
            placeholderText: "Type here to test"
            Accessible.name: "Type here to test"
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Mouse"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        ValueSlider {
            busy: im.busy
            settingId: "mouse-speed"
            label: "Pointer speed"
            highlight: App.highlight
            enabled: page.ready
            from: -1
            to: 1
            lowLabel: "Slower"
            highLabel: "Faster"
            value: page.mouse.sensitivity || 0
            onChosen: v => page.setMouse("sensitivity", v)
            Layout.fillWidth: true
        }
        SettingRow {
            settingId: "mouse-accel"
            label: "Pointer acceleration"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            UqComboBox {
                model: page.accelNames
                currentIndex: Math.max(0, page.accelCodes.indexOf(page.mouse.accel_profile || "default"))
                Accessible.name: "Pointer acceleration"
                onActivated: index => page.setMouse("accel_profile", page.accelCodes[index])
            }
        }
        SwitchRow {
            settingId: "mouse-natural-scroll"
            label: "Natural scrolling"
            highlight: App.highlight
            enabled: page.ready
            value: page.mouse.natural_scroll === true
            onChosen: v => page.setMouse("natural_scroll", v)
            Layout.fillWidth: true
        }
        SwitchRow {
            settingId: "mouse-left-handed"
            label: "Left-handed buttons"
            highlight: App.highlight
            enabled: page.ready
            value: page.mouse.left_handed === true
            onChosen: v => page.setMouse("left_handed", v)
            Layout.fillWidth: true
        }
        ValueSlider {
            busy: im.busy
            settingId: "mouse-scroll-speed"
            label: "Scroll speed"
            highlight: App.highlight
            enabled: page.ready
            from: 0.1
            to: 5
            stepSize: 0.1
            lowLabel: "Slower"
            highLabel: "Faster"
            value: page.mouse.scroll_factor || 1
            onChosen: v => page.setMouse("scroll_factor", v)
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Touchpad"
            font.bold: true
            visible: page.hasTouchpad
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        OverrideSwitch {
            settingId: "touchpad-natural-scroll"
            label: "Natural scrolling on the touchpad"
            highlight: App.highlight
            visible: page.hasTouchpad
            enabled: page.ready
            fallback: page.dTouchpad.natural_scroll === true
            value: page.saved(page.touchpad, "natural_scroll")
            onChosen: v => page.setTouchpad("natural_scroll", v)
            Layout.fillWidth: true
        }
        OverrideSwitch {
            settingId: "tap-to-click"
            label: "Tap to click"
            highlight: App.highlight
            visible: page.hasTouchpad
            enabled: page.ready
            fallback: page.dTouchpad.tap_to_click !== false
            value: page.saved(page.touchpad, "tap_to_click")
            onChosen: v => page.setTouchpad("tap_to_click", v)
            Layout.fillWidth: true
        }
        OverrideSwitch {
            settingId: "disable-while-typing"
            label: "Turn the touchpad off while typing"
            highlight: App.highlight
            visible: page.hasTouchpad
            enabled: page.ready
            fallback: page.dTouchpad.disable_while_typing !== false
            value: page.saved(page.touchpad, "disable_while_typing")
            onChosen: v => page.setTouchpad("disable_while_typing", v)
            Layout.fillWidth: true
        }
        OverrideSlider {
            busy: im.busy
            settingId: "touchpad-scroll-speed"
            label: "Touchpad scroll speed"
            highlight: App.highlight
            visible: page.hasTouchpad
            enabled: page.ready
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

        UqLabel {
            text: "One device"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SettingRow {
            settingId: "device-overrides"
            label: "Settings for one pointing device"
            highlight: App.highlight
            enabled: page.ready && page.pointers.length > 0
            Layout.fillWidth: true

            UqComboBox {
                id: devicePick
                model: ["Choose a device"].concat(page.pointers.map(p => p.name))
                currentIndex: 0
                Accessible.name: "Device to add"
            }
            QQC2.Button {
                text: "Add"
                enabled: devicePick.currentIndex > 0 && page.deviceSettings[page.pointers[devicePick.currentIndex - 1].name] === undefined
                onClicked: {
                    page.setDevice(page.pointers[devicePick.currentIndex - 1].name, "sensitivity", 0);
                    devicePick.currentIndex = 0;
                }
            }
        }
        Repeater {
            model: Object.keys(page.deviceSettings)

            ColumnLayout {
                id: dev
                required property string modelData
                readonly property var s: page.deviceSettings[modelData] || {}
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing

                RowLayout {
                    Layout.fillWidth: true
                    UqLabel {
                        text: dev.modelData
                        font.bold: true
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    QQC2.Button {
                        text: "Remove"
                        enabled: page.ready && !im.busy
                        onClicked: im.removeDevice(dev.modelData)
                    }
                }
                OverrideSlider {
                    busy: im.busy
                    label: "Pointer speed"
                    enabled: page.ready
                    from: -1
                    to: 1
                    fallback: page.mouse.sensitivity || 0
                    lowLabel: "Slower"
                    highLabel: "Faster"
                    value: page.saved(dev.s, "sensitivity")
                    onChosen: v => page.setDevice(dev.modelData, "sensitivity", v)
                    Layout.fillWidth: true
                }
                OverrideCombo {
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
                    label: "Natural scrolling"
                    enabled: page.ready
                    fallback: page.mouse.natural_scroll === true
                    value: page.saved(dev.s, "natural_scroll")
                    onChosen: v => page.setDevice(dev.modelData, "natural_scroll", v)
                    Layout.fillWidth: true
                }
                OverrideSwitch {
                    label: "Left-handed buttons"
                    enabled: page.ready
                    fallback: page.mouse.left_handed === true
                    value: page.saved(dev.s, "left_handed")
                    onChosen: v => page.setDevice(dev.modelData, "left_handed", v)
                    Layout.fillWidth: true
                }
                OverrideSlider {
                    busy: im.busy
                    label: "Scroll speed"
                    enabled: page.ready
                    from: 0.1
                    to: 5
                    stepSize: 0.1
                    fallback: page.mouse.scroll_factor || 1
                    lowLabel: "Slower"
                    highLabel: "Faster"
                    value: page.saved(dev.s, "scroll_factor")
                    onChosen: v => page.setDevice(dev.modelData, "scroll_factor", v)
                    Layout.fillWidth: true
                }
            }
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
