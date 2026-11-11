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

QQC2.ScrollView {
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

    Accessible.role: Accessible.Pane
    Accessible.name: "Sound"
    contentWidth: availableWidth

    SoundModel {
        id: sm
        onStateJsonChanged: page.showNews()
    }

    // A volume slider with its mute button. The slider shows the model's value again once a
    // change is done, so a refused one snaps back.
    component Volume: RowLayout {
        id: vol
        property var target: ({})
        property string what: ""
        spacing: Kirigami.Units.smallSpacing
        Layout.fillWidth: true

        QQC2.ToolButton {
            icon.name: vol.target.mute ? "audio-volume-muted" : "audio-volume-high"
            checkable: true
            checked: !!vol.target.mute
            enabled: page.idle
            Accessible.name: (vol.target.mute ? "Unmute " : "Mute ") + vol.what
            QQC2.ToolTip.text: "Mute"
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
            Accessible.name: "Volume of " + vol.what
            Layout.fillWidth: true
            onMoved: settle.restart()
            Keys.onReleased: settle.restart()
        }
        UqLabel {
            text: Math.round(slider.value) + "%"
            Layout.minimumWidth: Kirigami.Units.gridUnit * 2
            horizontalAlignment: Text.AlignRight
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

    // One output or input.
    component DeviceCard: Kirigami.AbstractCard {
        id: card
        required property var modelData
        property bool output: true
        readonly property var d: modelData
        Layout.fillWidth: true

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing
            RowLayout {
                Layout.fillWidth: true
                QQC2.RadioButton {
                    checked: !!card.d.default
                    enabled: page.idle
                    Accessible.name: "Use " + String(card.d.name) + (card.output ? " for sound" : " for recording")
                    onToggled: {
                        if (checked)
                            sm.setDefault(card.d.id);
                        checked = Qt.binding(() => !!card.d.default);
                    }
                }
                UqLabel {
                    text: String(card.d.name)
                    font.bold: !!card.d.default
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                QQC2.Button {
                    text: "Test"
                    icon.name: "media-playback-start"
                    visible: card.output
                    enabled: page.idle && page.snap.test === true
                    onClicked: sm.testSound(card.d.id)
                }
            }
            Volume {
                target: card.d
                what: String(card.d.name)
            }
        }
    }

    // One app playing or recording.
    component StreamCard: Kirigami.AbstractCard {
        id: scard
        required property var modelData
        property bool output: true
        readonly property var s: modelData
        readonly property var devices: scard.output ? page.outputs : page.inputs
        Layout.fillWidth: true

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing
            RowLayout {
                Layout.fillWidth: true
                Kirigami.Icon {
                    source: String(scard.s.icon || "")
                    fallback: scard.output ? "audio-volume-high" : "audio-input-microphone"
                    implicitWidth: Kirigami.Units.iconSizes.smallMedium
                    implicitHeight: Kirigami.Units.iconSizes.smallMedium
                    Accessible.ignored: true
                }
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    UqLabel {
                        text: String(scard.s.app)
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    UqLabel {
                        visible: String(scard.s.title) !== ""
                        text: String(scard.s.title)
                        opacity: 0.7
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }
                UqComboBox {
                    id: through
                    // First "follow the default", then each device.
                    readonly property var serials: [0].concat(scard.devices.map(d => d.serial))
                    readonly property int shown: scard.s.pinned ? Math.max(0, through.serials.indexOf(scard.s.through_serial)) : 0
                    model: [(scard.output ? "Default output" : "Default input") + (page.defaultOf(scard.devices) ? " (" + String(page.defaultOf(scard.devices).name) + ")" : "")].concat(scard.devices.map(d => String(d.name)))
                    currentIndex: through.shown
                    enabled: page.idle
                    implicitContentWidthPolicy: QQC2.ComboBox.WidestText
                    Accessible.name: (scard.output ? "Play " : "Record ") + String(scard.s.app) + (scard.output ? " through" : " from")
                    onActivated: index => {
                        sm.route(scard.s.id, through.serials[index]);
                        through.currentIndex = Qt.binding(() => through.shown);
                    }
                }
            }
            Volume {
                target: scard.s
                what: String(scard.s.app)
            }
        }
    }

    ColumnLayout {
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 44)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Sound"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        ErrorCard {
            objectName: "notAvailable"
            visible: page.loaded && page.snap.available === false
            title: "Not available"
            detail: String(page.snap.text || "")
            Layout.fillWidth: true
        }
        RuleMessage {
            objectName: "soundError"
            negative: true
            closable: false
            lines: sm.error !== "" ? ["That wasn't done: " + sm.error] : []
            Layout.fillWidth: true
        }
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }

        SettingRow {
            settingId: "sound-output"
            label: "Play sound through"
            highlight: App.highlight
            visible: page.ready
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            visible: page.ready && page.outputs.length === 0
            text: "No outputs found. Check that speakers or headphones are connected."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        Repeater {
            model: page.outputs
            delegate: DeviceCard {
                output: true
            }
        }
        UqLabel {
            visible: page.ready && page.snap.test === false
            text: "To play a test sound, install the sound-theme-freedesktop package."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }

        SettingRow {
            settingId: "sound-input"
            label: "Record sound from"
            highlight: App.highlight
            visible: page.ready
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            visible: page.ready && page.inputs.length === 0
            text: "No microphones found."
            opacity: 0.7
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        Repeater {
            model: page.inputs
            delegate: DeviceCard {
                output: false
            }
        }

        SettingRow {
            settingId: "sound-apps"
            label: page.playback.length + page.recording.length > 0 ? "Apps" : "Apps: none are playing or recording now"
            highlight: App.highlight
            visible: page.ready
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        Repeater {
            model: page.playback
            delegate: StreamCard {
                output: true
            }
        }
        Repeater {
            model: page.recording
            delegate: StreamCard {
                output: false
            }
        }

        SettingRow {
            settingId: "sound-profiles"
            label: "Sound card modes"
            highlight: App.highlight
            visible: page.ready && page.cards.length > 0
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        Repeater {
            model: page.cards
            delegate: SettingRow {
                id: cardRow
                required property var modelData
                readonly property var ids: modelData.profiles.map(p => p.index)
                label: String(modelData.name)
                Layout.fillWidth: true

                UqComboBox {
                    id: profileCombo
                    model: cardRow.modelData.profiles.map(p => String(p.name))
                    currentIndex: cardRow.ids.indexOf(cardRow.modelData.profile)
                    enabled: page.idle
                    implicitContentWidthPolicy: QQC2.ComboBox.WidestText
                    Accessible.name: "Mode of " + String(cardRow.modelData.name)
                    onActivated: index => {
                        sm.setProfile(cardRow.modelData.id, cardRow.ids[index]);
                        profileCombo.currentIndex = Qt.binding(() => cardRow.ids.indexOf(cardRow.modelData.profile));
                    }
                }
            }
        }
        UqLabel {
            visible: page.ready && page.cards.length > 0
            text: "A mode decides which plugs a sound card uses, such as speakers and microphone together, or the screen's speakers."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
