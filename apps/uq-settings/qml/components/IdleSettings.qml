// The idle stages: dim, lock, screen off and suspend (each off-able, in minutes), the locked
// screen-off delay (seconds) and whether playing media blocks the lock. Changes go out as
// `chosen(key, value)` with `key` a SetSetting key and `value` seconds, null or a boolean; uqd
// enforces the order dim < lock < screen off < suspend and the page shows its refusal.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

ColumnLayout {
    id: root

    /// The `idle` object of GetSettings.
    property var idle: ({})
    property string highlight
    property bool busy: false
    signal chosen(string key, var value)

    function secs(name) {
        return idle[name] === undefined ? null : idle[name];
    }
    function minutes(name) {
        const s = secs(name);
        return s === null ? null : Math.max(1, Math.round(s / 60));
    }
    function stage(key, v) {
        chosen("idle." + key, v === null ? null : v * 60);
    }

    spacing: Kirigami.Units.smallSpacing

    UqLabel {
        text: "Idle"
        font.bold: true
        Layout.topMargin: Kirigami.Units.largeSpacing
    }
    UqLabel {
        text: "Each stage comes after the one above it: dim, then lock, then screen off, then suspend on battery. A stage can be switched off."
        wrapMode: Text.Wrap
        opacity: 0.7
        Layout.fillWidth: true
    }
    OverrideSpin {
        busy: root.busy
        settingId: "idle-dim"
        label: "Dim the screen after"
        highlight: root.highlight
        enabled: root.enabled
        from: 1
        to: 1440
        fallback: 4
        unit: "min"
        useLabel: "Off"
        value: root.minutes("dim")
        onChosen: v => root.stage("dim", v)
        Layout.fillWidth: true
    }
    OverrideSpin {
        busy: root.busy
        settingId: "idle-lock"
        label: "Lock after"
        highlight: root.highlight
        enabled: root.enabled
        from: 1
        to: 1440
        fallback: 5
        unit: "min"
        useLabel: "Off"
        value: root.minutes("lock")
        onChosen: v => root.stage("lock", v)
        Layout.fillWidth: true
    }
    OverrideSpin {
        busy: root.busy
        settingId: "idle-screen-off"
        label: "Turn the screen off after"
        highlight: root.highlight
        enabled: root.enabled
        from: 1
        to: 1440
        fallback: 6
        unit: "min"
        useLabel: "Off"
        value: root.minutes("screen_off")
        onChosen: v => root.stage("screen_off", v)
        Layout.fillWidth: true
    }
    OverrideSpin {
        busy: root.busy
        settingId: "idle-suspend"
        label: "Suspend on battery after"
        highlight: root.highlight
        enabled: root.enabled
        from: 1
        to: 1440
        fallback: 20
        unit: "min"
        useLabel: "Off"
        value: root.minutes("suspend_battery")
        onChosen: v => root.stage("suspend_battery", v)
        Layout.fillWidth: true
    }
    SettingRow {
        settingId: "idle-locked-screen-off"
        label: "Turn the screen off after locking"
        highlight: root.highlight
        enabled: root.enabled
        Layout.fillWidth: true
        QQC2.SpinBox {
            id: lockedSpin
            from: 10
            to: 600
            stepSize: 5
            editable: true
            value: root.idle.locked_screen_off === undefined ? 30 : root.idle.locked_screen_off
            enabled: root.enabled
            Accessible.name: "Turn the screen off after locking"
            onValueModified: settle.restart()
        }
        UqLabel {
            text: "s"
        }
        Timer {
            id: settle
            interval: 400
            onTriggered: root.chosen("idle.locked_screen_off", lockedSpin.value)
        }
        Connections {
            target: root
            function onBusyChanged() {
                if (!root.busy)
                    lockedSpin.value = Qt.binding(() => root.idle.locked_screen_off === undefined ? 30 : root.idle.locked_screen_off);
            }
        }
    }
    SwitchRow {
        settingId: "idle-media"
        label: "Playing media keeps the screen from locking"
        highlight: root.highlight
        enabled: root.enabled
        value: root.idle.media_blocks_lock === true
        onChosen: v => root.chosen("idle.media_blocks_lock", v)
        Layout.fillWidth: true
    }
}
