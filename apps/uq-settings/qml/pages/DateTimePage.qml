// Date and time (spec §2.6): the time zone, setting the time from the network, the date and time
// by hand, and the 24-hour clock in the bar. Changes to the system clock ask for an administrator
// password; a cancelled prompt leaves them as they were. The 24-hour clock is uQ's own setting,
// so it needs uQ's settings service; everything else works without it.
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
    /// Microseconds for the typed local date and time, or -1 when they don't make a date.
    function typedUsec() {
        const d = dateField.text.match(/^(\d{4})-(\d{2})-(\d{2})$/);
        const t = timeField.text.match(/^(\d{1,2}):(\d{2})$/);
        if (!d || !t)
            return -1;
        const when = new Date(Number(d[1]), Number(d[2]) - 1, Number(d[3]), Number(t[1]), Number(t[2]), 0);
        if (when.getMonth() !== Number(d[2]) - 1 || when.getHours() !== Number(t[1]))
            return -1;
        return when.getTime() * 1000;
    }

    Accessible.role: Accessible.Pane
    Accessible.name: "Date and time"
    contentWidth: availableWidth

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

    ColumnLayout {
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 44)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Date and time"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        UqLabel {
            objectName: "now"
            text: Qt.formatDateTime(page.now, page.clock24 ? "dddd d MMMM yyyy, HH:mm" : "dddd d MMMM yyyy, h:mm AP")
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.2
        }
        ErrorCard {
            objectName: "notAvailable"
            visible: page.td.available === false
            title: "Not available"
            detail: String(page.td.text || "")
            Layout.fillWidth: true
        }
        RuleMessage {
            objectName: "dateError"
            negative: true
            closable: false
            lines: dm.error !== "" ? ["That wasn't done: " + dm.error] : []
            Layout.fillWidth: true
        }
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }

        SettingRow {
            settingId: "timezone"
            label: "Time zone: " + String(page.td.timezone || "unknown")
            highlight: App.highlight
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            QQC2.TextField {
                id: zoneFilter
                placeholderText: "Search, e.g. Berlin"
                enabled: page.idle
                Accessible.name: "Search time zones"
            }
            UqComboBox {
                id: zoneCombo
                model: page.shownZones
                currentIndex: page.shownZones.indexOf(page.td.timezone)
                enabled: page.idle && page.shownZones.length > 0
                Accessible.name: "Time zone"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 14
                onActivated: index => {
                    dm.setTimezone(String(page.shownZones[index]));
                    zoneCombo.currentIndex = Qt.binding(() => page.shownZones.indexOf(page.td.timezone));
                }
            }
        }

        SwitchRow {
            objectName: "ntp"
            settingId: "ntp"
            label: "Set the time from the network"
            highlight: App.highlight
            value: !!page.td.ntp
            enabled: page.idle && page.td.can_ntp !== false
            Layout.fillWidth: true
            onChosen: on => dm.setNtp(on)
        }
        UqLabel {
            visible: page.ready && !!page.td.ntp
            text: page.td.synced ? "The clock is in step with the network." : "Waiting to reach a time server."
            opacity: 0.7
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }

        SettingRow {
            settingId: "manual-time"
            label: "Date and time by hand"
            highlight: App.highlight
            enabled: page.idle && !page.td.ntp
            Layout.fillWidth: true

            QQC2.TextField {
                id: dateField
                placeholderText: "2026-11-11"
                inputMask: "9999-99-99"
                Accessible.name: "Date, as year-month-day"
                Component.onCompleted: text = Qt.formatDate(new Date(), "yyyy-MM-dd")
            }
            QQC2.TextField {
                id: timeField
                placeholderText: "14:30"
                inputMask: "99:99"
                Accessible.name: "Time, as hours and minutes"
                Component.onCompleted: text = Qt.formatTime(new Date(), "HH:mm")
            }
            QQC2.Button {
                text: "Set"
                icon.name: "document-save"
                enabled: page.typedUsec() > 0
                onClicked: dm.setTime(page.typedUsec())
            }
        }
        UqLabel {
            visible: page.ready && !!page.td.ntp
            text: "Turn off setting the time from the network to set it by hand."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }

        SwitchRow {
            objectName: "clock24"
            settingId: "datetime-clock-24h"
            label: "24-hour clock in the bar"
            highlight: App.highlight
            value: page.clock24
            enabled: !dm.busy && App.uqdUp && page.snap.clock_24h !== null && page.snap.clock_24h !== undefined
            Layout.fillWidth: true
            onChosen: on => dm.setClock24h(on)
        }
        UqLabel {
            visible: !App.uqdUp
            text: "The bar's clock can be changed when uQ's settings service is running."
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
