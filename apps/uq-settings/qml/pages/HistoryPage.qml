// History: two tabs. Restore points: save a new one, and each saved point as a card with Restore
// (after a confirmation) and, for the ones the user saved, Delete (after a confirmation too).
// Changes: every change uqd journalled, one card per day, each change with its page's icon and an
// Undo button, filtered by page with a row of page icons. It shares HistoryModel (and so the undo
// stack) with the window's Ctrl+Z and its "Changed this session" sheet; the window shows the
// confirmations and errors. Every summary and name is uqd's text or the user's, shown through
// UqLabel.
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
            return JSON.parse(HistoryModel.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var entries: snap.entries || []
    readonly property var points: snap.points || []
    readonly property bool ready: App.uqdUp
    /// The pages that make journalled changes, from the registry: `[{id, name, icon, domains}]`.
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

    function pageEntry(domain) {
        return page.filterPages.find(p => p.domains.indexOf(domain) >= 0) || null;
    }
    /// The id of the page that changes `domain`, "" when none does.
    function pageOf(domain) {
        const p = page.pageEntry(domain);
        return p ? p.id : "";
    }
    function pageName(domain) {
        const p = page.pageEntry(domain);
        return p ? p.name : "";
    }
    function pageIcon(domain) {
        const p = page.pageEntry(domain);
        return p && p.icon ? p.icon : "configure";
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
    /// The icon that says how a restore point was made.
    function kindIcon(kind) {
        if (kind === "manual")
            return "user-identity";
        switch (kind && kind.auto) {
        case "before_restore":
            return "document-revert";
        case "before_profile":
            return "bookmarks";
        case "before_import":
            return "document-import";
        case "new_version":
            return "system-software-install";
        }
        return "chronometer";
    }
    /// Read everything again; Main calls it when the window comes back to the front.
    function refresh() {
        HistoryModel.refresh();
    }

    Component.onCompleted: HistoryModel.refresh()

    Kirigami.PromptDialog {
        id: confirmRestore

        property string pointId

        title: "Restore these settings?"
        subtitle: "Every setting goes back to how it was when this restore point was saved. uQ saves a restore point of your current settings first, so you can come back: it appears first in the list, marked “Saved automatically before a restore”."
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: HistoryModel.restore(confirmRestore.pointId)
    }

    Kirigami.PromptDialog {
        id: confirmDelete

        property string pointId

        title: "Delete this restore point?"
        subtitle: "Your settings stay as they are now. This can't be undone."
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Delete"
                icon.name: "edit-delete"
                enabled: page.ready && !HistoryModel.busy
                onTriggered: {
                    HistoryModel.deleteRestorePoint(confirmDelete.pointId);
                    confirmDelete.close();
                }
            }
        ]
    }

    banners: [
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to see and undo changes."
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "Restore points"
        iconName: "document-revert"

        SettingsCard {
            title: "New restore point"
            iconName: "document-save"
            help: "uQ also saves one automatically before the first change of each day."

            SettingRow {
                settingId: "restore-points"
                label: "Name"
                highlight: App.highlight
                enabled: page.ready
                Layout.fillWidth: true

                QQC2.TextField {
                    id: pointName
                    objectName: "pointName"
                    maximumLength: 100
                    placeholderText: "Such as Before the party"
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
                objectName: "noPoints"
                text: "No restore points yet."
                visible: page.points.length === 0
                opacity: 0.7
                Layout.leftMargin: Kirigami.Units.largeSpacing
            }
        }

        // One card per restore point, newest first.
        Repeater {
            model: page.points
            delegate: SettingsCard {
                id: point

                required property var modelData
                readonly property bool manual: modelData.kind === "manual"

                objectName: "restorePoint"
                title: String(modelData.name)
                iconName: page.kindIcon(modelData.kind)
                help: page.kindText(modelData.kind)

                actions: [
                    QQC2.Button {
                        text: "Restore"
                        icon.name: "edit-undo"
                        Accessible.name: "Restore " + point.modelData.name
                        enabled: page.ready && !HistoryModel.busy
                        onClicked: {
                            confirmRestore.pointId = point.modelData.id;
                            confirmRestore.open();
                        }
                    },
                    QQC2.ToolButton {
                        objectName: "deletePoint"
                        text: "Delete"
                        icon.name: "edit-delete"
                        display: QQC2.AbstractButton.IconOnly
                        visible: point.manual
                        Accessible.name: "Delete " + point.modelData.name
                        enabled: page.ready && !HistoryModel.busy
                        QQC2.ToolTip.text: "Delete"
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                        onClicked: {
                            confirmDelete.pointId = point.modelData.id;
                            confirmDelete.open();
                        }
                    }
                ]

                RowLayout {
                    spacing: Kirigami.Units.smallSpacing
                    Layout.fillWidth: true

                    Kirigami.Icon {
                        source: "view-calendar-day"
                        implicitWidth: Kirigami.Units.iconSizes.small
                        implicitHeight: Kirigami.Units.iconSizes.small
                        Accessible.ignored: true
                    }
                    UqLabel {
                        text: page.dayLabel(point.modelData.time) + ", " + page.timeOf(point.modelData.time)
                        opacity: 0.7
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    SettingsTab {
        title: "Changes"
        iconName: "view-history"

        // Which page's changes: a row of page icons, "All" first.
        SettingsCard {
            wide: true

            SettingRow {
                settingId: "history"
                iconName: "view-filter"
                label: "Show changes made on"
                highlight: App.highlight
                enabled: page.ready
                Layout.fillWidth: true

                QQC2.ButtonGroup {
                    id: filterGroup
                }
                QQC2.ToolButton {
                    objectName: "filterAll"
                    text: "All pages"
                    icon.name: "view-list-icons"
                    display: QQC2.AbstractButton.TextBesideIcon
                    checkable: true
                    checked: page.filterPage === ""
                    QQC2.ButtonGroup.group: filterGroup
                    onClicked: page.filterPage = ""
                }
                Repeater {
                    model: page.filterPages
                    delegate: QQC2.ToolButton {
                        required property var modelData

                        objectName: "filter-" + modelData.id
                        icon.name: modelData.icon || "configure"
                        display: QQC2.AbstractButton.IconOnly
                        checkable: true
                        checked: page.filterPage === modelData.id
                        QQC2.ButtonGroup.group: filterGroup
                        Accessible.name: modelData.name
                        QQC2.ToolTip.text: modelData.name
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                        onClicked: page.filterPage = modelData.id
                    }
                }
            }
            UqLabel {
                objectName: "noChanges"
                text: page.filterPage === "" ? "No changes in the last 30 days." : "No changes on this page in the last 30 days."
                visible: page.days.length === 0
                opacity: 0.7
                Layout.leftMargin: Kirigami.Units.largeSpacing
            }
        }

        // The timeline: one card per day, each change with its page's icon and time.
        Repeater {
            model: page.days
            delegate: SettingsCard {
                id: day

                required property var modelData

                title: day.modelData.day
                iconName: "view-calendar-day"

                Repeater {
                    model: day.modelData.entries
                    delegate: RowLayout {
                        id: entry

                        required property var modelData

                        spacing: Kirigami.Units.largeSpacing
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.smallSpacing

                        UqLabel {
                            text: page.timeOf(entry.modelData.time)
                            opacity: 0.7
                            Layout.alignment: Qt.AlignTop
                            Layout.topMargin: Kirigami.Units.smallSpacing / 2
                        }
                        Kirigami.Icon {
                            source: page.pageIcon(entry.modelData.domain)
                            implicitWidth: Kirigami.Units.iconSizes.smallMedium
                            implicitHeight: Kirigami.Units.iconSizes.smallMedium
                            Layout.alignment: Qt.AlignTop
                            Accessible.name: page.pageName(entry.modelData.domain)

                            HoverHandler {
                                id: iconHover
                            }
                            QQC2.ToolTip.text: page.pageName(entry.modelData.domain)
                            QQC2.ToolTip.visible: iconHover.hovered && page.pageName(entry.modelData.domain) !== ""
                            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                        }
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
                                    text: "Changed"
                                    opacity: 0.7
                                }
                                OriginLabel {
                                    origin: entry.modelData.origin
                                    profileNames: page.snap.profiles || ({})
                                    opacity: 0.7
                                }
                            }
                        }
                        QQC2.ToolButton {
                            text: "Undo"
                            icon.name: "edit-undo"
                            display: QQC2.AbstractButton.IconOnly
                            Accessible.name: "Undo " + entry.modelData.summary
                            enabled: page.ready && !HistoryModel.busy
                            Layout.alignment: Qt.AlignTop
                            QQC2.ToolTip.text: "Undo"
                            QQC2.ToolTip.visible: hovered
                            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                            onClicked: HistoryModel.undo(entry.modelData.id, false)
                        }
                    }
                }
            }
        }
    }
}
