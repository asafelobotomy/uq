// Date and time (spec §2.6): the time zone, setting the time from the network, the date and time
// by hand, and the bar's clock format. Changes to the system clock ask for an administrator
// password; a cancelled prompt leaves them as they were. The clock format is uQ's own setting,
// so it needs uQ's settings service; everything else works without it.
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
            return JSON.parse(dm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var td: snap.timedate || ({})
    readonly property bool ready: td.available === true
    readonly property bool idle: page.ready && !dm.busy
    readonly property var zones: td.zones || []
    readonly property var shownZones: {
        const q = zoneFilter.text.trim().toLowerCase().replace(/ /g, "_");
        return q === "" ? page.zones : page.zones.filter(z => String(z).toLowerCase().indexOf(q) >= 0);
    }
    readonly property bool clock24: snap.clock_24h !== false
    readonly property bool clockKnown: snap.clock_24h !== null && snap.clock_24h !== undefined
    property date now: new Date()
    property int noticeSeen: 0

    function refresh() {
        dm.refresh();
    }
    function showNews() {
        const notice = page.snap.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.show(notice.text !== "" ? [notice.text] : []);
        }
    }
    /// Microseconds for the chosen local date and time, or -1 when they don't make a date.
    function typedUsec() {
        const d = dateField.text.match(/^(\d{4})-(\d{2})-(\d{2})$/);
        if (!d)
            return -1;
        const when = new Date(Number(d[1]), Number(d[2]) - 1, Number(d[3]), hourSpin.value, minuteSpin.value, 0);
        if (when.getMonth() !== Number(d[2]) - 1)
            return -1;
        return when.getTime() * 1000;
    }
    /// The zone's place name, "Europe/London" as "London".
    function city(zone) {
        const s = String(zone || "");
        return s.substring(s.lastIndexOf("/") + 1).replace(/_/g, " ");
    }

    DateTimeModel {
        id: dm
        onStateJsonChanged: page.showNews()
    }
    Timer {
        interval: 1000
        repeat: true
        running: page.visible
        onTriggered: page.now = new Date()
    }

    banners: [
        ErrorCard {
            objectName: "notAvailable"
            visible: page.td.available === false
            title: "Not available"
            detail: String(page.td.text || "")
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "dateError"
            negative: true
            closable: false
            lines: dm.error !== "" ? ["That wasn't done: " + dm.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "Date and time"
        iconName: "preferences-system-time"

        // The clock itself, large: what the computer thinks the time is, and where.
        SettingsCard {
            wide: true
            RowLayout {
                spacing: Kirigami.Units.largeSpacing * 2
                Layout.fillWidth: true

                UqLabel {
                    objectName: "now"
                    text: Qt.formatTime(page.now, page.clock24 ? "HH:mm" : "h:mm")
                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 4
                    font.weight: Font.Light
                    Accessible.name: "The time is " + Qt.formatTime(page.now, page.clock24 ? "HH:mm" : "h:mm AP")
                }
                // Takes what is left beside the time and wraps in it (a narrow window).
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    UqLabel {
                        visible: !page.clock24
                        text: Qt.formatTime(page.now, "AP")
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.4
                    }
                    UqLabel {
                        text: Qt.formatDate(page.now, "dddd d MMMM yyyy")
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.4
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                    Flow {
                        spacing: Kirigami.Units.smallSpacing
                        Layout.fillWidth: true
                        Kirigami.Icon {
                            source: "globe"
                            implicitWidth: Kirigami.Units.iconSizes.small
                            implicitHeight: Kirigami.Units.iconSizes.small
                            Accessible.ignored: true
                        }
                        UqLabel {
                            objectName: "zoneNow"
                            text: page.city(page.td.timezone) || "Unknown time zone"
                            opacity: 0.8
                        }
                        Item {
                            visible: syncIcon.visible
                            width: Kirigami.Units.largeSpacing - Kirigami.Units.smallSpacing
                            height: 1
                        }
                        Kirigami.Icon {
                            id: syncIcon
                            visible: page.ready && !!page.td.ntp
                            source: page.td.synced ? "emblem-checked" : "chronometer"
                            implicitWidth: Kirigami.Units.iconSizes.small
                            implicitHeight: Kirigami.Units.iconSizes.small
                            Accessible.ignored: true
                        }
                        UqLabel {
                            objectName: "syncState"
                            visible: page.ready && !!page.td.ntp
                            text: page.td.synced ? "In step with the network" : "Reaching a time server…"
                            opacity: 0.8
                        }
                    }
                }
            }
        }

        SettingsCard {
            title: "Time zone"
            iconName: "globe"

            SettingRow {
                settingId: "timezone"
                label: "Zone"
                highlight: App.highlight
                Layout.fillWidth: true

                QQC2.TextField {
                    id: zoneFilter
                    placeholderText: "Find a city"
                    enabled: page.idle
                    Accessible.name: "Search time zones"
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 9
                }
                UqComboBox {
                    id: zoneCombo
                    model: page.shownZones
                    currentIndex: page.shownZones.indexOf(page.td.timezone)
                    enabled: page.idle && page.shownZones.length > 0
                    Accessible.name: "Time zone"
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 13
                    onActivated: index => {
                        dm.setTimezone(String(page.shownZones[index]));
                        zoneCombo.currentIndex = Qt.binding(() => page.shownZones.indexOf(page.td.timezone));
                    }
                }
            }
        }

        SettingsCard {
            title: "Clock in the bar"
            iconName: "clock"

            // The two formats, drawn with the time now: pick the one that looks right.
            SettingRow {
                objectName: "clock24"
                settingId: "datetime-clock-24h"
                label: "Format"
                highlight: App.highlight
                enabled: !dm.busy && App.uqdUp && page.clockKnown
                help: App.uqdUp ? "" : "Needs uQ's settings service, which isn't running."
                Layout.fillWidth: true

                QQC2.ButtonGroup {
                    id: clockGroup
                }
                ChoiceButton {
                    objectName: "clock24On"
                    label: Qt.formatTime(page.now, "HH:mm")
                    checkable: true
                    checked: page.clock24
                    QQC2.ButtonGroup.group: clockGroup
                    Accessible.name: "24-hour clock"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: "24-hour"
                    onClicked: if (!page.clock24) dm.setClock24h(true)
                }
                ChoiceButton {
                    objectName: "clock24Off"
                    label: Qt.formatTime(page.now, "h:mm AP")
                    checkable: true
                    checked: !page.clock24
                    QQC2.ButtonGroup.group: clockGroup
                    Accessible.name: "12-hour clock"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: "12-hour"
                    onClicked: if (page.clock24) dm.setClock24h(false)
                }
            }
        }

        SettingsCard {
            title: "Setting the time"
            iconName: "chronometer"

            SwitchRow {
                objectName: "ntp"
                settingId: "ntp"
                iconName: "network-wireless"
                label: "Automatically, from the network"
                highlight: App.highlight
                value: !!page.td.ntp
                enabled: page.idle && page.td.can_ntp !== false
                Layout.fillWidth: true
                onChosen: on => page.ready && dm.setNtp(on)
            }
            SettingRow {
                settingId: "manual-time"
                iconName: "document-edit"
                label: "By hand"
                help: page.td.ntp ? "Turn off setting the time automatically first." : ""
                highlight: App.highlight
                enabled: page.idle && !page.td.ntp
                Layout.fillWidth: true

                QQC2.TextField {
                    id: dateField
                    inputMask: "9999-99-99"
                    Accessible.name: "Date, as year-month-day"
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                    Component.onCompleted: text = Qt.formatDate(new Date(), "yyyy-MM-dd")
                }
                QQC2.SpinBox {
                    id: hourSpin
                    from: 0
                    to: 23
                    editable: true
                    Accessible.name: "Hour"
                    Component.onCompleted: value = new Date().getHours()
                }
                UqLabel {
                    text: ":"
                }
                QQC2.SpinBox {
                    id: minuteSpin
                    from: 0
                    to: 59
                    editable: true
                    textFromValue: v => (v < 10 ? "0" : "") + v
                    Accessible.name: "Minute"
                    Component.onCompleted: value = new Date().getMinutes()
                }
                QQC2.Button {
                    text: "Set"
                    icon.name: "dialog-ok-apply"
                    enabled: page.typedUsec() > 0
                    onClicked: dm.setTime(page.typedUsec())
                }
            }
        }
    }
}
