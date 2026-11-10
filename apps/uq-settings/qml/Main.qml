// The Settings window: the category sidebar on the left, the current page on the right.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "components"

Kirigami.ApplicationWindow {
    id: root

    readonly property var pages: JSON.parse(App.pagesJson || "[]")
    /// The settings history (HistoryModel): entries, restore points, the last confirmation and a
    /// pending conflict.
    readonly property var history: {
        try {
            return JSON.parse(HistoryModel.stateJson);
        } catch (e) {
            return {};
        }
    }
    /// The last confirmation and conflict shown, so each is shown once.
    property int noticeSeen: 0
    property int conflictSeen: 0
    /// A text field has the focus: Ctrl+Z and Ctrl+Shift+Z are its own undo and redo then.
    readonly property bool editingText: activeFocusItem !== null && activeFocusItem.cursorPosition !== undefined

    function showCurrent() {
        const page = root.pages.find(p => p.id === App.page);
        if (!page)
            return;
        // A plugin page is loaded by PluginPage, which turns a QML error into a card.
        if (page.plugin)
            stack.replace(null, Qt.resolvedUrl("PluginPage.qml"), { source: page.qml, theme: JSON.parse(App.themeJson || "{}"), highlight: Qt.binding(() => App.highlight) });
        else
            stack.replace(null, Qt.resolvedUrl(page.qml));
    }

    // Coming back to the window re-reads the page: user.lua or the live session may have changed
    // meanwhile, and the rows' sources should say so. Pages that read from uqd have `refresh()`.
    onActiveChanged: {
        const page = stack.currentItem;
        if (root.active && page && typeof page.refresh === "function")
            page.refresh();
    }

    title: "uQ Settings"
    visible: true
    width: Kirigami.Units.gridUnit * 50
    height: Kirigami.Units.gridUnit * 34
    minimumWidth: Kirigami.Units.gridUnit * 30
    minimumHeight: Kirigami.Units.gridUnit * 20

    header: QQC2.ToolBar {
        RowLayout {
            anchors.fill: parent
            SessionChanges {
                entries: root.history.entries || []
                sessionStart: root.history.session_start || 0
                enabled: App.uqdUp
                onUndoRequested: id => HistoryModel.undo(id, false)
            }
            Item {
                Layout.fillWidth: true
            }
            Search {
                id: search
                searchFunction: query => App.search(query)
                onActivated: (pageId, settingId) => App.showPage(pageId, settingId)
                Layout.preferredWidth: Kirigami.Units.gridUnit * 18
            }
        }
    }

    // Undo and redo this app's own changes of this session (see pages::history).
    Shortcut {
        sequences: [StandardKey.Undo]
        enabled: !root.editingText
        onActivated: HistoryModel.undoLast()
    }
    Shortcut {
        sequences: [StandardKey.Redo, "Ctrl+Shift+Z"]
        enabled: !root.editingText
        onActivated: HistoryModel.redoLast()
        // StandardKey.Redo is often Ctrl+Shift+Z itself; the same key twice in one Shortcut is
        // reported as ambiguous instead of activated.
        onActivatedAmbiguously: HistoryModel.redoLast()
    }

    // uqd refused an undo because the setting changed again since: ask before forcing it.
    Kirigami.PromptDialog {
        id: conflictDialog

        property string entryId

        title: "Undo anyway?"
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Undo anyway"
                icon.name: "edit-undo"
                onTriggered: {
                    HistoryModel.undo(conflictDialog.entryId, true);
                    conflictDialog.close();
                }
            }
        ]

        // uqd's message, as plain text (in place of the subtitle).
        UqLabel {
            id: conflictMessage
            objectName: "conflictMessage"
            wrapMode: Text.Wrap
        }
    }

    // Each history action's confirmation, and its error when one was refused.
    footer: ColumnLayout {
        spacing: 0

        Kirigami.InlineMessage {
            id: toast
            objectName: "historyToast"
            type: Kirigami.MessageType.Positive
            showCloseButton: true
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "historyError"
            visible: HistoryModel.error !== ""
            title: "That wasn't done"
            detail: HistoryModel.error
            Layout.fillWidth: true
        }
    }
    Timer {
        id: toastTimer
        interval: 4000
        onTriggered: toast.visible = false
    }
    Connections {
        target: HistoryModel
        function onStateJsonChanged() {
            let h = {};
            try {
                h = JSON.parse(HistoryModel.stateJson);
            } catch (e) {
                return;
            }
            const notice = h.notice;
            if (notice && notice.seq > root.noticeSeen) {
                root.noticeSeen = notice.seq;
                // A fixed phrase from pages::history, not uqd's text.
                toast.text = notice.text;
                toast.visible = true;
                toastTimer.restart();
            }
            const conflict = h.conflict;
            if (conflict && conflict.seq > root.conflictSeen) {
                root.conflictSeen = conflict.seq;
                conflictDialog.entryId = conflict.id;
                conflictMessage.text = conflict.message;
                conflictDialog.open();
            }
        }
    }

    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None
    pageStack.initialPage: Kirigami.Page {
        padding: 0
        globalToolBarStyle: Kirigami.ApplicationHeaderStyle.None

        RowLayout {
            anchors.fill: parent
            spacing: 0

            Sidebar {
                id: sidebar
                Layout.fillHeight: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 12
                pages: root.pages
                current: App.page
                focus: true
                onChosen: pageId => App.showPage(pageId, "")
            }
            Kirigami.Separator {
                Layout.fillHeight: true
            }
            QQC2.StackView {
                id: stack
                Layout.fillWidth: true
                Layout.fillHeight: true
                activeFocusOnTab: true
                Accessible.role: Accessible.Pane
                Accessible.name: "Page"
                Component.onCompleted: root.showCurrent()
            }
        }
    }

    Connections {
        target: App
        function onPageChanged() {
            root.showCurrent();
        }
        function onUqdUpChanged() {
            if (App.uqdUp)
                HistoryModel.refresh();
        }
        function onRaiseRequested() {
            root.show();
            root.raise();
            root.requestActivate();
        }
    }

    // `--screenshots <dir>`: show each page, save it as <dir>/<page>.png, then quit.
    Timer {
        id: shots

        property int step: 0
        readonly property var ids: root.pages.map(p => p.id)

        interval: 600
        repeat: true
        running: App.screenshotDir !== ""
        onTriggered: {
            if (step > 0 && !App.grabWindow(App.screenshotDir + "/" + ids[step - 1] + ".png"))
                console.warn("uq-settings: couldn't save " + ids[step - 1] + ".png");
            if (step === ids.length) {
                Qt.quit();
                return;
            }
            App.showPage(ids[step], "");
            step += 1;
        }
    }
}
