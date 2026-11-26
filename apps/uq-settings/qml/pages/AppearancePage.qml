// Appearance: theme, light or dark, automatic switching, and the font, cursor, icon and Qt style
// overrides, on four tabs (Theme, Fonts, Cursor and icons, Import and export). Everything is read from and written to uqd through AppearanceModel; a refused change
// shows uqd's message and the controls go back to the last good value. The theme row also adds,
// makes, edits, exports and deletes themes through ThemesModel. Theme names, authors and folder
// names come from files anyone can write, so they are shown through UqLabel only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"
import "appearance"

SettingsPage {
    id: page

    readonly property var snap: {
        try {
            return JSON.parse(am.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var appearance: snap.appearance || {}
    readonly property var schedule: snap.schedule || { mode: "off" }
    /// Where each row's live value comes from, by setting id (uqd's Explain); {} when unknown.
    readonly property var sources: snap.sources || {}
    readonly property bool ready: App.uqdUp
    readonly property bool idle: page.ready && !tm.busy
    readonly property var tsnap: {
        try {
            return JSON.parse(tm.stateJson);
        } catch (e) {
            return {};
        }
    }
    /// The themes `[{id, name, user, active, missing}]`; until ThemesModel has answered, the
    /// list AppearanceModel read.
    readonly property var themes: {
        if (page.tsnap.themes)
            return page.tsnap.themes;
        return (page.snap.themes || []).map(t => ({ id: t.id, name: t.name, user: t.user === true, active: false, missing: false }));
    }
    readonly property var skipped: page.tsnap.skipped || []
    readonly property string currentThemeName: {
        const t = page.themes.find(x => x.id === page.snap.theme);
        return t ? String(t.name) : "";
    }
    /// The last notice, delete, import and conflict shown, so each is shown once.
    property int noticeSeen: 0
    property int deletedSeen: 0
    property int importedSeen: 0
    property int conflictSeen: 0
    /// The last file chosen for Import…, and whether its window look was asked for, kept for Replace.
    property string importUrl
    property bool importLook: false
    /// A New or Edit was asked for: the editor opens when the session comes back.
    property bool editorWanted: false
    /// The rendered theme's tokens: what "Use theme" means right now, for seeding overrides.
    readonly property var tokens: {
        try {
            return JSON.parse(App.themeJson || "{}");
        } catch (e) {
            return {};
        }
    }
    readonly property string uiFont: page.appearance.ui_font || (page.tokens.fonts && page.tokens.fonts.ui) || Kirigami.Theme.defaultFont.family
    readonly property string monoFont: page.appearance.mono_font || (page.tokens.fonts && page.tokens.fonts.mono) || Kirigami.Theme.fixedWidthFont.family
    readonly property int fontSize: page.appearance.font_size || page.themeFontSize
    readonly property int themeFontSize: (page.tokens.fonts && page.tokens.fonts.size) || Kirigami.Theme.defaultFont.pointSize
    function field(name, value) {
        am.setAppearance(name, JSON.stringify(value));
    }
    /// Where a row's live value comes from (`{source, detail}`), or null: see SettingRow.
    function sourceOf(settingId) {
        return page.sources[settingId] ?? null;
    }
    /// The colours to draw a theme's tile with: each theme's own from the theme list, or the
    /// rendered tokens for the theme in use.
    function swatchesOf(id) {
        const t = page.themeById(id);
        if (t && t.swatches && t.swatches.length >= 4)
            return t.swatches;
        // The theme in use, when it has no pre-rendered colours (one of the user's own).
        const c = page.tokens.colors;
        if (!c || page.tokens.id !== id)
            return [];
        return [c.bg, c.surface, c.text, c.accent, c.success, c.warning, c.error].filter(x => !!x);
    }
    function themeById(id) {
        return page.themes.find(t => t.id === id) || null;
    }
    /// Read everything again; Main calls it when the window comes back to the front.
    function refresh() {
        am.refresh();
        tm.refresh();
    }
    /// Show what the last theme request said: a notice, a delete's result, an import's "Use it
    /// now", a conflict to ask about, or the editor's session to open.
    function showNews() {
        let s = {};
        try {
            s = JSON.parse(tm.stateJson);
        } catch (e) {
            return;
        }
        if (s.notice && s.notice.seq > page.noticeSeen) {
            page.noticeSeen = s.notice.seq;
            themeNotice.notices = s.notice.text !== "" ? [s.notice.text] : [];
            themeNotice.dismissed = false;
            noticeTimer.restart();
        }
        if (s.deleted && s.deleted.seq > page.deletedSeen) {
            page.deletedSeen = s.deleted.seq;
            const lines = ["The theme was deleted." + (s.deleted.switched ? " uQ switched to its default theme." : "")];
            for (const name of s.deleted.profiles || [])
                lines.push("The " + String(name) + " profile uses this theme. Pick another theme for it.");
            themeNotice.notices = lines;
            themeNotice.dismissed = false;
            noticeTimer.stop();
        }
        if (s.imported && s.imported.seq > page.importedSeen) {
            page.importedSeen = s.imported.seq;
            addedCard.themeId = String(s.imported.id);
            addedCard.themeName = String(s.imported.name);
            addedCard.unusedLook = s.imported.unusedLook === true;
            addedCard.dismissed = false;
        }
        if (s.conflict && s.conflict.seq > page.conflictSeen) {
            page.conflictSeen = s.conflict.seq;
            conflictDialog.url = page.importUrl;
            conflictMessage.text = String(s.conflict.message);
            conflictDialog.open();
        }
        if (s.editor && page.editorWanted) {
            page.editorWanted = false;
            themeEditor.start(s.editor);
        }
    }

    Component.onCompleted: {
        am.refresh();
        tm.refresh();
    }

    AppearanceModel {
        id: am
    }
    ThemesModel {
        id: tm
        onStateJsonChanged: page.showNews()
        // A refused New or Edit never opens the editor later.
        onErrorChanged: {
            if (tm.error !== "")
                page.editorWanted = false;
        }
    }
    Timer {
        id: noticeTimer
        interval: 6000
        onTriggered: themeNotice.dismissed = true
    }

    Connections {
        target: App
        function onUqdUpChanged() {
            if (App.uqdUp) {
                am.refresh();
                tm.refresh();
            }
        }
    }

    ThemeEditor {
        id: themeEditor
        preview: draft => tm.preview(draft)
        canSave: page.idle
        busy: tm.busy
        errorNow: () => tm.error
        onSaved: draft => {
            tm.save(draft);
            // No request went out (no session bus, or the draft couldn't be read): say so here.
            if (!tm.busy)
                themeEditor.finishSave(tm.error !== "" ? tm.error : "uQ isn't running");
        }
        onClosed: tm.closeEditor()
    }

    // An imported file has the id of a theme that exists. The message (uqd's, without its
    // question) names it; the title and the button ask.
    Kirigami.PromptDialog {
        id: conflictDialog

        /// The URL the file was imported from. Not the conflict's `path`: that is decoded
        /// already and would be decoded again on the way back.
        property string url

        title: "Import a theme"
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Replace"
                icon.name: "document-replace"
                enabled: page.idle
                onTriggered: {
                    tm.importFrom(conflictDialog.url, true, page.importLook);
                    conflictDialog.close();
                }
            }
        ]

        ColumnLayout {
            UqLabel {
                id: conflictMessage
                objectName: "conflictMessage"
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Replace it?"
            }
        }
    }

    Kirigami.PromptDialog {
        id: confirmDelete

        property string themeId
        property string themeName
        property bool active

        title: "Delete theme"
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Delete"
                icon.name: "edit-delete"
                enabled: page.idle
                onTriggered: {
                    tm.remove(confirmDelete.themeId);
                    confirmDelete.close();
                }
            }
        ]

        ColumnLayout {
            UqLabel {
                objectName: "deleteWhat"
                text: "Delete " + confirmDelete.themeName + "? This can't be undone."
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            UqLabel {
                visible: confirmDelete.active
                text: "uQ will switch to its default theme first."
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
        }
    }

    FileDialog {
        id: importDialog
        title: "Import a theme"
        fileMode: FileDialog.OpenFile
        nameFilters: ["uQ themes (*.uqtheme)"] // jargon-ok: the file filter shows the file's extension
        onAccepted: {
            page.importUrl = String(selectedFile);
            page.importLook = importExportTab.importLookChecked;
            tm.importFrom(page.importUrl, false, page.importLook);
        }
    }
    FileDialog {
        id: exportDialog

        property string themeId

        title: "Export a theme"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "uqtheme" // jargon-ok: the file extension
        nameFilters: ["uQ themes (*.uqtheme)"] // jargon-ok: the file filter shows the file's extension
        onAccepted: tm.exportTo(exportDialog.themeId, String(selectedFile), importExportTab.exportLookChecked)
    }

    banners: [
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to change these settings."
            Layout.fillWidth: true
        },
        ErrorCard {
            objectName: "changeError"
            visible: am.error !== ""
            title: "That change wasn't applied"
            detail: am.error
            Layout.fillWidth: true
        },
        ErrorCard {
            objectName: "themeError"
            visible: tm.error !== "" && !themeEditor.opened
            title: "That theme change wasn't made"
            detail: tm.error
            Layout.fillWidth: true
        },
        ProfileMessage {
            id: themeNotice
            objectName: "themeNotice"
            Layout.fillWidth: true
        },
        // "Added <name>." with Use it now. Built from UqLabels, as ProfileMessage is: the name
        // comes from the file.
        Rectangle {
            id: addedCard
            objectName: "addedCard"
            property string themeId
            property string themeName
            property bool unusedLook: false
            property bool dismissed: true
            visible: !dismissed
            implicitHeight: addedRow.implicitHeight + Kirigami.Units.largeSpacing * 2
            radius: Kirigami.Units.cornerRadius
            color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g,
                           Kirigami.Theme.highlightColor.b, 0.12)
            border.width: 1
            border.color: Kirigami.Theme.highlightColor
            Layout.fillWidth: true
            Accessible.role: Accessible.AlertMessage
            Accessible.name: "Added " + themeName

            RowLayout {
                id: addedRow
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.smallSpacing

                Kirigami.Icon {
                    source: "document-import"
                    implicitWidth: Kirigami.Units.iconSizes.smallMedium
                    implicitHeight: Kirigami.Units.iconSizes.smallMedium
                    Layout.alignment: Qt.AlignTop
                    Accessible.ignored: true
                }
                ColumnLayout {
                    spacing: Kirigami.Units.smallSpacing
                    Layout.fillWidth: true
                    UqLabel {
                        objectName: "addedLine"
                        text: "Added " + addedCard.themeName + "."
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                    UqLabel {
                        objectName: "unusedLookLine"
                        visible: addedCard.unusedLook
                        text: "Its window gaps, corners and effects weren't used. Import it again with that box ticked to use them."
                        wrapMode: Text.Wrap
                        opacity: 0.8
                        Layout.fillWidth: true
                    }
                }
                QQC2.Button {
                    objectName: "useItNow"
                    text: "Use it now"
                    icon.name: "dialog-ok-apply"
                    enabled: page.idle
                    Layout.alignment: Qt.AlignTop
                    onClicked: {
                        tm.useTheme(addedCard.themeId);
                        addedCard.dismissed = true;
                    }
                }
                QQC2.ToolButton {
                    text: "Close"
                    icon.name: "dialog-close"
                    display: QQC2.AbstractButton.IconOnly
                    Accessible.name: "Close"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: "Close"
                    Layout.alignment: Qt.AlignTop
                    onClicked: addedCard.dismissed = true
                }
            }
        }
    ]

    ThemeTab {
        am: am
        confirmDelete: confirmDelete
        exportDialog: exportDialog
        page: page
        tm: tm
    }

    FontsTab {
        am: am
        page: page
    }

    CursorIconsTab {
        am: am
        page: page
    }

    ImportExportTab {
        id: importExportTab
        exportDialog: exportDialog
        importDialog: importDialog
        page: page
        tm: tm
    }
}
