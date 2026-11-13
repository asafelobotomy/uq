// Sound (spec §2.6): where sound plays and where it's recorded from, how loud each one is, a
// sound card's profile, each app's volume and output, and a test sound. Changes are made by the
// sound service, which remembers them. Device and app names come from the devices and apps and
// are shown through UqLabel (plain text) only.
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
            return JSON.parse(sm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property bool loaded: snap.notice !== undefined
    readonly property bool ready: snap.available === true
    readonly property bool idle: page.ready && !sm.busy
    readonly property var outputs: snap.outputs || []
    readonly property var inputs: snap.inputs || []
    readonly property var playback: snap.playback || []
    readonly property var recording: snap.recording || []
    readonly property var cards: snap.cards || []
    property int noticeSeen: 0

    function refresh() {
        sm.refresh();
    }
    function showNews() {
        const notice = page.snap.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.show(notice.text !== "" ? [notice.text] : []);
        }
    }
    function defaultOf(list) {
        return list.find(d => d.default) || null;
    }
    /// A picture of the device, guessed from its name: headphones, a screen, speakers.
    function deviceIcon(d, output) {
        const n = String(d.name || "").toLowerCase();
        if (/headset/.test(n))
            return "audio-headset";
        if (/head ?phone|earbud|airpod|buds/.test(n))
            return "audio-headphones";
        if (!output)
            return /webcam|camera|cam\b/.test(n) ? "camera-web" : "audio-input-microphone";
        if (/hdmi|displayport|\bdp\b|monitor|display|tv\b/.test(n))
            return "video-display";
        if (/digital|s\/?pdif|optical|iec958/.test(n))
            return "media-optical-audio";
        return "audio-speakers";
    }

    SoundModel {
        id: sm
        onStateJsonChanged: page.showNews()
    }

    banners: [
        ErrorCard {
            objectName: "notAvailable"
            visible: page.loaded && page.snap.available === false
            title: "Not available"
            detail: String(page.snap.text || "")
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "soundError"
            negative: true
            closable: false
            lines: sm.error !== "" ? ["That wasn't done: " + sm.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }
    ]

    // A volume: the mute button, a slider and the level in large type. The slider shows the
    // model's value again once a change is done, so a refused one snaps back.
    component VolumeRow: SettingRow {
        id: vol
        property var target: ({})
        property string what: ""
        wideControls: true
        Accessible.name: "Volume of " + vol.what
        Layout.fillWidth: true

        QQC2.ToolButton {
            icon.name: vol.target.mute ? "audio-volume-muted" : (slider.value < 34 ? "audio-volume-low" : slider.value < 67 ? "audio-volume-medium" : "audio-volume-high")
            checkable: true
            checked: !!vol.target.mute
            enabled: page.idle
            Accessible.name: (vol.target.mute ? "Unmute " : "Mute ") + vol.what
            QQC2.ToolTip.text: vol.target.mute ? "Unmute" : "Mute"
            QQC2.ToolTip.visible: hovered
            onToggled: {
                sm.setMute(vol.target.id, checked);
                checked = Qt.binding(() => !!vol.target.mute);
            }
        }
        QQC2.Slider {
            id: slider
            from: 0
            to: 100
            stepSize: 1
            value: vol.target.volume || 0
            enabled: page.ready
            opacity: vol.target.mute ? 0.5 : 1
            Accessible.name: "Volume of " + vol.what
            Layout.fillWidth: true
            Layout.minimumWidth: Kirigami.Units.gridUnit * 8
            onMoved: settle.restart()
            Keys.onReleased: settle.restart()
        }
        UqLabel {
            text: Math.round(slider.value) + "%"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.3
            opacity: vol.target.mute ? 0.5 : 1
            horizontalAlignment: Text.AlignRight
            Layout.minimumWidth: Kirigami.Units.gridUnit * 3
        }
        Timer {
            id: settle
            interval: 150
            onTriggered: {
                sm.setVolume(vol.target.id, Math.round(slider.value));
                slider.value = Qt.binding(() => vol.target.volume || 0);
            }
        }
    }

    // One output or input: its picture and name, "Default", a test sound, and its volume.
    component DeviceCard: SettingsCard {
        id: card
        required property var modelData
        required property int index
        property bool output: true
        property string firstId: ""
        readonly property var d: modelData
        readonly property bool isDefault: !!card.d.default

        title: String(card.d.name)
        iconName: page.deviceIcon(card.d, card.output)
        help: card.output && page.snap.test === false ? "To play a test sound, install the sound-theme-freedesktop package." : ""
        border.width: card.isDefault ? 2 : 1
        border.color: card.isDefault ? Kirigami.Theme.highlightColor : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.08)

        actions: [
            QQC2.ToolButton {
                icon.name: "media-playback-start"
                visible: card.output
                enabled: page.idle && page.snap.test === true
                display: QQC2.AbstractButton.IconOnly
                Accessible.name: "Play a test sound on " + String(card.d.name)
                QQC2.ToolTip.text: "Play a test sound"
                QQC2.ToolTip.visible: hovered
                onClicked: sm.testSound(card.d.id)
            },
            QQC2.RadioButton {
                text: "Default"
                checked: card.isDefault
                enabled: page.idle
                Accessible.name: "Use " + String(card.d.name) + (card.output ? " for sound" : " for recording")
                onToggled: {
                    if (checked)
                        sm.setDefault(card.d.id);
                    checked = Qt.binding(() => card.isDefault);
                }
            }
        ]

        VolumeRow {
            settingId: card.index === 0 ? card.firstId : ""
            highlight: App.highlight
            target: card.d
            what: String(card.d.name)
        }
    }

    // One app playing or recording, on one line: what it is, where it goes, how loud.
    component StreamRow: SettingRow {
        id: srow
        required property var modelData
        required property int index
        property bool output: true
        readonly property var s: modelData
        readonly property var devices: srow.output ? page.outputs : page.inputs

        settingId: srow.index === 0 && srow.output ? "sound-apps" : ""
        highlight: App.highlight
        iconName: String(srow.s.icon || "") || (srow.output ? "applications-multimedia" : "audio-input-microphone")
        label: String(srow.s.app)
        Layout.fillWidth: true

        UqLabel {
            visible: String(srow.s.title) !== ""
            text: String(srow.s.title)
            opacity: 0.7
            elide: Text.ElideRight
            Layout.maximumWidth: Kirigami.Units.gridUnit * 8
        }
        UqComboBox {
            id: through
            // First "follow the default", then each device.
            readonly property var serials: [0].concat(srow.devices.map(d => d.serial))
            readonly property int shown: srow.s.pinned ? Math.max(0, through.serials.indexOf(srow.s.through_serial)) : 0
            model: [(srow.output ? "Default output" : "Default input") + (page.defaultOf(srow.devices) ? " (" + String(page.defaultOf(srow.devices).name) + ")" : "")].concat(srow.devices.map(d => String(d.name)))
            currentIndex: through.shown
            enabled: page.idle
            Accessible.name: (srow.output ? "Play " : "Record ") + String(srow.s.app) + (srow.output ? " through" : " from")
            Layout.preferredWidth: Kirigami.Units.gridUnit * 11
            onActivated: index => {
                sm.route(srow.s.id, through.serials[index]);
                through.currentIndex = Qt.binding(() => through.shown);
            }
        }
        QQC2.ToolButton {
            icon.name: srow.s.mute ? "audio-volume-muted" : "audio-volume-high"
            checkable: true
            checked: !!srow.s.mute
            enabled: page.idle
            Accessible.name: (srow.s.mute ? "Unmute " : "Mute ") + String(srow.s.app)
            QQC2.ToolTip.text: srow.s.mute ? "Unmute" : "Mute"
            QQC2.ToolTip.visible: hovered
            onToggled: {
                sm.setMute(srow.s.id, checked);
                checked = Qt.binding(() => !!srow.s.mute);
            }
        }
        QQC2.Slider {
            id: appSlider
            from: 0
            to: 100
            stepSize: 1
            value: srow.s.volume || 0
            enabled: page.ready
            opacity: srow.s.mute ? 0.5 : 1
            Accessible.name: "Volume of " + String(srow.s.app)
            Layout.preferredWidth: Kirigami.Units.gridUnit * 9
            onMoved: appSettle.restart()
            Keys.onReleased: appSettle.restart()
        }
        UqLabel {
            text: Math.round(appSlider.value) + "%"
            horizontalAlignment: Text.AlignRight
            Layout.minimumWidth: Kirigami.Units.gridUnit * 2
        }
        Timer {
            id: appSettle
            interval: 150
            onTriggered: {
                sm.setVolume(srow.s.id, Math.round(appSlider.value));
                appSlider.value = Qt.binding(() => srow.s.volume || 0);
            }
        }
    }

    SettingsTab {
        title: "Output"
        iconName: "audio-speakers"

        Repeater {
            model: page.outputs
            delegate: DeviceCard {
                output: true
                firstId: "sound-output"
            }
        }
        SettingsCard {
            visible: page.ready && page.outputs.length === 0
            wide: true
            SettingRow {
                settingId: page.outputs.length === 0 ? "sound-output" : ""
                iconName: "audio-speakers"
                label: "No speakers or headphones found. Check that they're plugged in."
                highlight: App.highlight
                Layout.fillWidth: true
            }
        }
    }

    SettingsTab {
        title: "Input"
        iconName: "audio-input-microphone"

        Repeater {
            model: page.inputs
            delegate: DeviceCard {
                output: false
                firstId: "sound-input"
            }
        }
        SettingsCard {
            visible: page.ready && page.inputs.length === 0
            wide: true
            SettingRow {
                settingId: page.inputs.length === 0 ? "sound-input" : ""
                iconName: "mic-off"
                label: "No microphones found."
                highlight: App.highlight
                Layout.fillWidth: true
            }
        }
    }

    SettingsTab {
        title: "Apps"
        iconName: "applications-multimedia"

        SettingsCard {
            title: "Playing"
            iconName: "media-playback-start"
            wide: true

            SettingRow {
                visible: page.playback.length === 0
                settingId: page.playback.length === 0 ? "sound-apps" : ""
                iconName: "media-playback-stop"
                label: "Nothing is playing now."
                highlight: App.highlight
                opacity: 0.7
                Layout.fillWidth: true
            }
            Repeater {
                model: page.playback
                delegate: StreamRow {
                    output: true
                }
            }
        }
        SettingsCard {
            title: "Recording"
            iconName: "audio-input-microphone"
            wide: true

            SettingRow {
                visible: page.recording.length === 0
                iconName: "mic-off"
                label: "Nothing is recording now."
                opacity: 0.7
                Layout.fillWidth: true
            }
            Repeater {
                model: page.recording
                delegate: StreamRow {
                    output: false
                }
            }
        }
    }

    SettingsTab {
        title: "Sound cards"
        iconName: "audio-card"
        available: page.cards.length > 0

        SettingsCard {
            title: "Modes"
            iconName: "audio-card"
            help: "A mode decides which plugs a sound card uses, such as speakers and microphone together, or the screen's speakers."
            wide: true

            Repeater {
                model: page.cards
                delegate: SettingRow {
                    id: cardRow
                    required property var modelData
                    required property int index
                    readonly property var ids: modelData.profiles.map(p => p.index)
                    settingId: index === 0 ? "sound-profiles" : ""
                    highlight: App.highlight
                    iconName: "audio-card"
                    label: String(modelData.name)
                    Layout.fillWidth: true

                    UqComboBox {
                        id: profileCombo
                        model: cardRow.modelData.profiles.map(p => String(p.name))
                        currentIndex: cardRow.ids.indexOf(cardRow.modelData.profile)
                        enabled: page.idle
                        Accessible.name: "Mode of " + String(cardRow.modelData.name)
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 22
                        onActivated: index => {
                            sm.setProfile(cardRow.modelData.id, cardRow.ids[index]);
                            profileCombo.currentIndex = Qt.binding(() => cardRow.ids.indexOf(cardRow.modelData.profile));
                        }
                    }
                }
            }
        }
    }
}
