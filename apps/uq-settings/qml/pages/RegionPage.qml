// Region (spec §2.6): the system language, the formats (how dates, numbers, money, measurements
// and paper look), and the keyboard layout of the text console. Each change asks for an
// administrator password; a cancelled prompt leaves it as it was. Locale and layout names are
// shown through UqLabel (plain text) only.
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
            return JSON.parse(rm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var loc: snap.localed || ({})
    readonly property bool ready: loc.available === true
    readonly property bool idle: page.ready && !rm.busy
    readonly property var locales: snap.locales || []
    readonly property var keymaps: snap.keymaps || []
    property int noticeSeen: 0

    function refresh() {
        rm.refresh();
    }
    function showNews() {
        const notice = page.snap.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.show(notice.text !== "" ? [notice.text] : []);
        }
    }
    /// The list with `current` added when the computer has it set but doesn't list it.
    function withCurrent(list, current) {
        return current && list.indexOf(current) < 0 ? list.concat([current]) : list;
    }

    Accessible.role: Accessible.Pane
    Accessible.name: "Region"
    contentWidth: availableWidth

    RegionModel {
        id: rm
        onStateJsonChanged: page.showNews()
    }

    ColumnLayout {
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 44)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Region"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        UqLabel {
            text: "These apply to the whole computer, including the login screen. Changes need an administrator password."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "notAvailable"
            visible: page.loc.available === false
            title: "Not available"
            detail: String(page.loc.text || "")
            Layout.fillWidth: true
        }
        RuleMessage {
            objectName: "regionError"
            negative: true
            closable: false
            lines: rm.error !== "" ? ["That wasn't done: " + rm.error] : []
            Layout.fillWidth: true
        }
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }

        SettingRow {
            settingId: "language"
            label: "Language"
            highlight: App.highlight
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            UqComboBox {
                id: langCombo
                readonly property var items: page.withCurrent(page.locales, page.loc.language)
                model: langCombo.items
                currentIndex: langCombo.items.indexOf(page.loc.language)
                enabled: page.idle
                Accessible.name: "Language"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 14
                onActivated: index => {
                    rm.setLanguage(String(langCombo.items[index]));
                    langCombo.currentIndex = Qt.binding(() => langCombo.items.indexOf(page.loc.language));
                }
            }
        }
        SettingRow {
            settingId: "formats"
            label: "Formats: dates, numbers, money and measurements"
            highlight: App.highlight
            Layout.fillWidth: true

            UqComboBox {
                id: formatsCombo
                readonly property var items: page.withCurrent(page.locales, page.loc.formats)
                model: formatsCombo.items
                currentIndex: formatsCombo.items.indexOf(page.loc.formats)
                enabled: page.idle
                Accessible.name: "Formats"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 14
                onActivated: index => {
                    rm.setFormats(String(formatsCombo.items[index]));
                    formatsCombo.currentIndex = Qt.binding(() => formatsCombo.items.indexOf(page.loc.formats));
                }
            }
        }
        UqLabel {
            text: "Choosing the same as the language makes the formats follow it."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            visible: page.ready && page.locales.length === 0
            text: "No languages are set up yet. Add them to the locale list and generate them, then come back."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        SettingRow {
            settingId: "console-keymap"
            label: "Keyboard layout for the text console"
            highlight: App.highlight
            Layout.fillWidth: true

            UqComboBox {
                id: keymapCombo
                readonly property var items: page.withCurrent(page.keymaps, page.loc.keymap)
                model: keymapCombo.items
                currentIndex: keymapCombo.items.indexOf(page.loc.keymap)
                enabled: page.idle
                Accessible.name: "Keyboard layout for the text console"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 14
                onActivated: index => {
                    rm.setKeymap(String(keymapCombo.items[index]));
                    keymapCombo.currentIndex = Qt.binding(() => keymapCombo.items.indexOf(page.loc.keymap));
                }
            }
        }
        UqLabel {
            text: "Your desktop's keyboard layouts are on the Input page."
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
