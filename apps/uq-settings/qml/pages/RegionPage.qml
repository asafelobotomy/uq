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

SettingsPage {
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
    /// Qt's locale for a system locale name ("de_DE.UTF-8", "sr_RS@latin"), or null when Qt
    /// doesn't know it.
    function qtLocale(code) {
        const base = String(code || "").split(".")[0].split("@")[0];
        if (base === "" || base === "C" || base === "POSIX")
            return null;
        const l = Qt.locale(base);
        return l.name === "C" ? null : l;
    }
    /// "Deutsch (Deutschland) · de_DE.UTF-8": the language in its own words, then the name.
    function localeLabel(code) {
        const l = page.qtLocale(code);
        if (!l || !l.nativeLanguageName)
            return String(code);
        const where = l.nativeTerritoryName || "";
        return l.nativeLanguageName + (where !== "" ? " (" + where + ")" : "") + " · " + code;
    }
    readonly property var formatLocale: page.qtLocale(page.loc.formats)
    property date now: new Date()

    RegionModel {
        id: rm
        onStateJsonChanged: page.showNews()
    }

    Timer {
        interval: 30000
        repeat: true
        running: page.visible
        onTriggered: page.now = new Date()
    }

    banners: [
        ErrorCard {
            objectName: "notAvailable"
            visible: page.loc.available === false
            title: "Not available"
            detail: String(page.loc.text || "")
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "regionError"
            negative: true
            closable: false
            lines: rm.error !== "" ? ["That wasn't done: " + rm.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "Region"
        iconName: "preferences-desktop-locale"

        SettingsCard {
            title: "Language and formats"
            iconName: "globe"
            help: "These apply to the whole computer, including the login screen. Changes ask for an administrator password."

            UqLabel {
                visible: page.ready && page.locales.length === 0
                text: "No languages are set up yet. Add them to the locale list and generate them, then come back."
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
            }
            SettingRow {
                settingId: "language"
                iconName: "preferences-desktop-locale"
                label: "Language"
                highlight: App.highlight
                Layout.fillWidth: true

                UqComboBox {
                    id: langCombo
                    readonly property var items: page.withCurrent(page.locales, page.loc.language)
                    model: langCombo.items.map(c => page.localeLabel(c))
                    currentIndex: langCombo.items.indexOf(page.loc.language)
                    enabled: page.idle
                    Accessible.name: "Language"
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 18
                    onActivated: index => {
                        rm.setLanguage(String(langCombo.items[index]));
                        langCombo.currentIndex = Qt.binding(() => langCombo.items.indexOf(page.loc.language));
                    }
                }
            }
            SettingRow {
                settingId: "formats"
                iconName: "format-number-percent"
                label: "Formats"
                help: "How dates, numbers, money and measurements look. Choosing the same as the language makes them follow it."
                highlight: App.highlight
                Layout.fillWidth: true

                UqComboBox {
                    id: formatsCombo
                    readonly property var items: page.withCurrent(page.locales, page.loc.formats)
                    model: formatsCombo.items.map(c => page.localeLabel(c))
                    currentIndex: formatsCombo.items.indexOf(page.loc.formats)
                    enabled: page.idle
                    Accessible.name: "Formats"
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 18
                    onActivated: index => {
                        rm.setFormats(String(formatsCombo.items[index]));
                        formatsCombo.currentIndex = Qt.binding(() => formatsCombo.items.indexOf(page.loc.formats));
                    }
                }
            }
            // How things look with these formats, now.
            GridLayout {
                objectName: "formatSamples"
                visible: page.formatLocale !== null
                columns: 2
                columnSpacing: Kirigami.Units.largeSpacing * 2
                rowSpacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing * 2 + Kirigami.Units.iconSizes.smallMedium
                Layout.bottomMargin: Kirigami.Units.smallSpacing

                Repeater {
                    model: {
                        const l = page.formatLocale;
                        if (!l)
                            return [];
                        return [
                            { icon: "view-calendar-day", name: "Date", text: page.now.toLocaleDateString(l, Locale.ShortFormat) + "  ·  " + page.now.toLocaleDateString(l, Locale.LongFormat) },
                            { icon: "chronometer", name: "Time", text: page.now.toLocaleTimeString(l, Locale.ShortFormat) },
                            { icon: "format-number-percent", name: "Number", text: Number(1234567.89).toLocaleString(l, "f", 2) },
                            { icon: "format-currency", name: "Money", text: Number(1234.5).toLocaleCurrencyString(l) },
                            { icon: "measure", name: "Measurements", text: l.measurementSystem === Locale.MetricSystem ? "Metric (cm, kg, °C)" : "Imperial (in, lb, °F)" }
                        ];
                    }
                    delegate: RowLayout {
                        required property var modelData
                        spacing: Kirigami.Units.smallSpacing
                        Layout.fillWidth: true
                        Accessible.role: Accessible.StaticText
                        Accessible.name: modelData.name + ": " + modelData.text
                        Kirigami.Icon {
                            source: modelData.icon
                            implicitWidth: Kirigami.Units.iconSizes.small
                            implicitHeight: Kirigami.Units.iconSizes.small
                            opacity: 0.8
                            Accessible.ignored: true
                            QQC2.ToolTip.visible: iconHover.hovered
                            QQC2.ToolTip.text: modelData.name
                            HoverHandler {
                                id: iconHover
                            }
                        }
                        UqLabel {
                            text: modelData.text
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                }
            }
        }

        SettingsCard {
            title: "Text console"
            iconName: "utilities-terminal"
            help: "The screen you get without the desktop, such as after Ctrl+Alt+F3."
            actions: [
                QQC2.ToolButton {
                    objectName: "openInput"
                    text: "Desktop layouts"
                    icon.name: "input-keyboard"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: "Your desktop's keyboard layouts are on the Input page"
                    onClicked: App.showPage("input", "")
                }
            ]

            SettingRow {
                settingId: "console-keymap"
                iconName: "input-keyboard"
                label: "Keyboard layout"
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
        }
    }
}
