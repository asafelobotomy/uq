// History: every change uqd journalled, grouped by day and filterable by page, each with an Undo
// button; the restore points, with Restore (after a confirmation) and Delete for the ones the user
// saved; and a field to save a new one. It shares HistoryModel (and so the undo stack) with the
// window's Ctrl+Z and its "Changed this session" sheet; the window shows the confirmations and
// errors. Every summary and name is uqd's text or the user's, shown through UqLabel.
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
            return JSON.parse(HistoryModel.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var entries: snap.entries || []
    readonly property var points: snap.points || []
    readonly property bool ready: App.uqdUp
    /// The pages that make journalled changes, from the registry: `[{id, name, domains}]`.
    readonly property var filterPages: {
        try {
            return JSON.parse(App.pagesJson || "[]").filter(p => (p.domains || []).length > 0);
        } catch (e) {
            return [];
        }
    }
    /// The page whose changes are shown; "" for all.
    property string filterPage: ""
    /// The shown entries grouped by local day, newest first: `[{day, entries}]`.
    readonly property var days: {
        const shown = page.filterPage === "" ? page.entries : page.entries.filter(e => page.pageOf(e.domain) === page.filterPage);
        const groups = [];
        for (const e of shown) {
            const day = page.dayLabel(e.time);
            if (groups.length === 0 || groups[groups.length - 1].day !== day)
                groups.push({ day: day, entries: [] });
            groups[groups.length - 1].entries.push(e);
        }
        return groups;
    }

    /// The id of the page that changes `domain`, "" when none does.
    function pageOf(domain) {
        const p = page.filterPages.find(p => p.domains.indexOf(domain) >= 0);
        return p ? p.id : "";
    }
    function pageName(domain) {
        const p = page.filterPages.find(p => p.domains.indexOf(domain) >= 0);
        return p ? p.name : "";
    }
    /// "Today", "Yesterday" or the local date of unix time `t`.
    function dayLabel(t) {
        const d = new Date(t * 1000);
        const today = new Date();
        const yesterday = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 1);
        if (d.toDateString() === today.toDateString())
            return "Today";
        if (d.toDateString() === yesterday.toDateString())
            return "Yesterday";
        return d.toLocaleDateString(Qt.locale(), Locale.LongFormat);
    }
    function timeOf(t) {
        return Qt.formatTime(new Date(t * 1000), "hh:mm");
    }
    /// How a restore point was made: "manual", or `{auto: reason}`.
    function kindText(kind) {
        if (kind === "manual")
            return "Saved by you";
        switch (kind && kind.auto) {
        case "daily":
            return "Saved automatically before the day's first change";
        case "before_restore":
            return "Saved automatically before a restore";
        case "before_profile":
            return "Saved automatically before a profile was applied";
        case "before_import":
            return "Saved automatically before an import";
        case "new_version":
            return "Saved automatically when uQ was updated";
        }
        return "Saved automatically";
    }
    /// Read everything again; Main calls it when the window comes back to the front.
    function refresh() {
        HistoryModel.refresh();
    }

    Component.onCompleted: HistoryModel.refresh()
    Accessible.role: Accessible.Pane
    Accessible.name: "History"
    contentWidth: availableWidth

    Kirigami.PromptDialog {
        id: confirmRestore

        property string pointId

        title: "Restore these settings?"
        subtitle: "Every setting goes back to how it was when this restore point was saved. uQ saves a restore point of your current settings first, so you can come back."
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: HistoryModel.restore(confirmRestore.pointId)
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
            text: "History"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to see and undo changes."
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Restore points"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SettingRow {
            settingId: "restore-points"
            label: "Save a restore point"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            QQC2.TextField {
                id: pointName
                objectName: "pointName"
                maximumLength: 100
                placeholderText: "Name, such as Before the party"
                Accessible.name: "Name of the new restore point"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 14
                onAccepted: saveButton.clicked()
            }
            QQC2.Button {
                id: saveButton
                objectName: "saveRestorePoint"
                text: "Save"
                icon.name: "document-save"
                Accessible.name: "Save a restore point"
                enabled: page.ready && !HistoryModel.busy && pointName.text.trim() !== ""
                onClicked: {
                    if (!enabled)
                        return;
                    HistoryModel.saveRestorePoint(pointName.text);
                    pointName.clear();
                }
            }
        }
        UqLabel {
            text: "No restore points yet. uQ saves one automatically before the first change of each day."
            visible: page.points.length === 0
            opacity: 0.7
            wrapMode: Text.Wrap
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
            Layout.fillWidth: true
        }
        Repeater {
            model: page.points
            delegate: RowLayout {
                id: point

                required property var modelData
                readonly property bool manual: modelData.kind === "manual"

                spacing: Kirigami.Units.largeSpacing
                Layout.leftMargin: Kirigami.Units.largeSpacing * 2
                Layout.fillWidth: true

                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    UqLabel {
                        text: point.modelData.name
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    UqLabel {
                        text: page.dayLabel(point.modelData.time) + ", " + page.timeOf(point.modelData.time) + ". " + page.kindText(point.modelData.kind)
                        opacity: 0.7
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                }
                QQC2.Button {
                    text: "Restore"
                    icon.name: "edit-undo"
                    Accessible.name: "Restore " + point.modelData.name
                    enabled: page.ready && !HistoryModel.busy
                    onClicked: {
                        confirmRestore.pointId = point.modelData.id;
                        confirmRestore.open();
                    }
                }
                QQC2.Button {
                    text: "Delete"
                    icon.name: "edit-delete"
                    visible: point.manual
                    Accessible.name: "Delete " + point.modelData.name
                    enabled: page.ready && !HistoryModel.busy
                    onClicked: HistoryModel.deleteRestorePoint(point.modelData.id)
                }
            }
        }

        UqLabel {
            text: "Changes"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing * 2
        }
        SettingRow {
            settingId: "history"
            label: "Show changes made on"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            UqComboBox {
                id: filter
                objectName: "pageFilter"
                model: ["All pages"].concat(page.filterPages.map(p => p.name))
                currentIndex: 0
                Accessible.name: "Show changes made on"
                onActivated: index => page.filterPage = index === 0 ? "" : page.filterPages[index - 1].id
            }
        }
        UqLabel {
            text: page.filterPage === "" ? "No changes in the last 30 days." : "No changes on this page in the last 30 days."
            visible: page.days.length === 0
            opacity: 0.7
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        }
        Repeater {
            model: page.days
            delegate: ColumnLayout {
                id: day

                required property var modelData

                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true

                UqLabel {
                    text: day.modelData.day
                    font.bold: true
                    opacity: 0.8
                    Accessible.role: Accessible.Heading
                    Layout.topMargin: Kirigami.Units.largeSpacing
                }
                Repeater {
                    model: day.modelData.entries
                    delegate: RowLayout {
                        id: entry

                        required property var modelData

                        spacing: Kirigami.Units.largeSpacing
                        Layout.leftMargin: Kirigami.Units.largeSpacing * 2
                        Layout.fillWidth: true

                        ColumnLayout {
                            spacing: 0
                            Layout.fillWidth: true
                            UqLabel {
                                text: entry.modelData.summary
                                wrapMode: Text.Wrap
                                Layout.fillWidth: true
                            }
                            RowLayout {
                                spacing: Kirigami.Units.smallSpacing
                                UqLabel {
                                    text: page.timeOf(entry.modelData.time) + (page.pageName(entry.modelData.domain) !== "" ? ", " + page.pageName(entry.modelData.domain) + "," : ",")
                                    opacity: 0.7
                                }
                                OriginLabel {
                                    origin: entry.modelData.origin
                                    opacity: 0.7
                                }
                            }
                        }
                        QQC2.Button {
                            text: "Undo"
                            icon.name: "edit-undo"
                            Accessible.name: "Undo " + entry.modelData.summary
                            enabled: page.ready && !HistoryModel.busy
                            onClicked: HistoryModel.undo(entry.modelData.id, false)
                        }
                    }
                }
            }
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
