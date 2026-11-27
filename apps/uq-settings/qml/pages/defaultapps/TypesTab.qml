// The File types tab, or the Links tab (`links`): every type an installed app opens, searchable
// and by group, with the app that opens it now; a row opens the picker. Only the first rows that
// match are listed, so a system with thousands of types stays quick.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    id: tab

    property bool links: false
    property var types: []
    property var apps: ({})
    property bool idle: true

    signal pick(var entry)

    readonly property int shownMax: 150
    readonly property var groups: {
        const seen = [];
        const out = [{
                id: "",
                label: "All types"
            }];
        for (const t of tab.types) {
            if ((t.group === "links") !== tab.links || seen.indexOf(t.group) >= 0)
                continue;
            seen.push(t.group);
            out.push({
                id: t.group,
                label: String(t.group_label)
            });
        }
        return out;
    }
    property string group: ""
    readonly property var matching: {
        const q = search.text.trim().toLowerCase();
        return tab.types.filter(t => {
            if ((t.group === "links") !== tab.links)
                return false;
            if (tab.group !== "" && t.group !== tab.group)
                return false;
            if (q === "")
                return true;
            const app = tab.apps[t.current] || {};
            return String(t.mime).toLowerCase().indexOf(q) >= 0 || String(t.label).toLowerCase().indexOf(q) >= 0 || String(app.name || "").toLowerCase().indexOf(q) >= 0;
        });
    }

    SettingsCard {
        title: tab.links ? "Links" : "File types"
        iconName: tab.links ? "link" : "document-properties"
        help: tab.links ? "Which app opens each kind of link, such as email addresses, magnet links or meeting invitations." : "Which app opens each type of file. The Apps tab sets whole groups of these at once."
        wide: true

        SettingRow {
            settingId: tab.links ? "link-types" : "file-types"
            iconName: "search"
            label: tab.matching.length === 1 ? "1 type" : tab.matching.length + " types"
            highlight: App.highlight
            Layout.fillWidth: true

            QQC2.TextField {
                id: search
                objectName: tab.links ? "linkSearch" : "typeSearch"
                placeholderText: "Search"
                Accessible.name: "Search"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 12
            }
            UqComboBox {
                id: groupBox
                visible: !tab.links
                model: tab.groups.map(g => g.label)
                Accessible.name: "Group"
                onActivated: i => tab.group = tab.groups[i].id
            }
        }
        Kirigami.Separator {
            Layout.fillWidth: true
        }
        UqLabel {
            visible: tab.matching.length === 0
            text: "Nothing matches."
            opacity: 0.7
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        Repeater {
            model: tab.matching.slice(0, tab.shownMax)
            delegate: QQC2.ItemDelegate {
                id: typeRow
                required property var modelData
                readonly property var app: tab.apps[typeRow.modelData.current] || null
                objectName: "type_" + typeRow.modelData.mime
                enabled: tab.idle
                Accessible.name: String(typeRow.modelData.label || typeRow.modelData.mime)
                Layout.fillWidth: true
                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        UqLabel {
                            text: String(typeRow.modelData.label || typeRow.modelData.mime)
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        UqLabel {
                            text: String(typeRow.modelData.mime)
                            visible: !!typeRow.modelData.label
                            opacity: 0.6
                            font: Kirigami.Theme.smallFont
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                    RowLayout {
                        spacing: Kirigami.Units.smallSpacing
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 11
                        Layout.maximumWidth: Kirigami.Units.gridUnit * 11
                        UqAppIcon {
                            visible: typeRow.app !== null
                            name: typeRow.app ? String(typeRow.app.icon || "") : ""
                            label: typeRow.app ? String(typeRow.app.name || "") : ""
                            size: Kirigami.Units.iconSizes.small
                        }
                        UqLabel {
                            text: typeRow.app ? String(typeRow.app.name) : typeRow.modelData.current ? String(typeRow.modelData.current) + " (not installed)" : "Not chosen"
                            opacity: typeRow.app ? 1 : 0.6
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                    UqLabel {
                        text: typeRow.modelData.source === "system" ? "System" : typeRow.modelData.source === "override" ? "Desktop list" : ""
                        opacity: 0.6
                        font: Kirigami.Theme.smallFont
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 4
                    }
                    Kirigami.Icon {
                        source: "go-next"
                        implicitWidth: Kirigami.Units.iconSizes.small
                        implicitHeight: Kirigami.Units.iconSizes.small
                        Accessible.ignored: true
                    }
                }
                onClicked: tab.pick(typeRow.modelData)
            }
        }
        UqLabel {
            visible: tab.matching.length > tab.shownMax
            text: "Showing the first " + tab.shownMax + ". Search to find the rest."
            opacity: 0.7
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
    }
}
