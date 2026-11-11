// Appearance: theme, light or dark, automatic switching, and the font, cursor, icon and Qt style
// overrides. Everything is read from and written to uqd through AppearanceModel; a refused change
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

QQC2.ScrollView {
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
    function field(name, value) {
        am.setAppearance(name, JSON.stringify(value));
    }
    /// Where a row's live value comes from (`{source, detail}`), or null: see SettingRow.
    function sourceOf(settingId) {
        return page.sources[settingId] ?? null;
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
    Accessible.role: Accessible.Pane
    Accessible.name: "Appearance"
    contentWidth: availableWidth

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
            page.importLook = importLookBox.checked;
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
        onAccepted: tm.exportTo(exportDialog.themeId, String(selectedFile), exportLookBox.checked)
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
            text: "Appearance"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to change these settings."
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "changeError"
            visible: am.error !== ""
            title: "That change wasn't applied"
            detail: am.error
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "themeError"
            visible: tm.error !== "" && !themeEditor.opened
            title: "That theme change wasn't made"
            detail: tm.error
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Theme"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SettingRow {
            settingId: "theme"
            label: "Theme"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
            ColumnLayout {
                spacing: Kirigami.Units.smallSpacing
                Layout.preferredWidth: Kirigami.Units.gridUnit * 22
                Flow {
                    spacing: Kirigami.Units.smallSpacing
                    Layout.fillWidth: true
                    Repeater {
                        model: page.themes
                        ThemeChip {
                            required property var modelData
                            theme: Object.assign({}, modelData, { active: modelData.id === page.snap.theme })
                            onChosen: id => am.setTheme(id, page.snap.variant || "dark")
                            onEdit: id => {
                                page.editorWanted = true;
                                tm.openEditor(id, true);
                            }
                            onExportTheme: id => {
                                exportDialog.themeId = id;
                                exportDialog.open();
                            }
                            onRemove: id => {
                                const t = page.themeById(id);
                                confirmDelete.themeId = id;
                                confirmDelete.themeName = t ? String(t.name) : id;
                                confirmDelete.active = id === page.snap.theme;
                                confirmDelete.open();
                            }
                        }
                    }
                }
                UqLabel {
                    objectName: "rootIsLink"
                    visible: page.tsnap.rootIsLink === true
                    text: "Your themes folder is a link, so uQ won't use or change it."
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                UqLabel {
                    objectName: "skippedLine"
                    visible: page.skipped.length > 0
                    text: page.skipped.length === 1 ? "1 theme in your themes folder couldn't be used." : page.skipped.length + " themes in your themes folder couldn't be used."
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                QQC2.ToolButton {
                    id: skippedToggle
                    objectName: "skippedToggle"
                    visible: page.skipped.length > 0
                    text: "Details"
                    icon.name: checked ? "arrow-down" : "arrow-right"
                    checkable: true
                    display: QQC2.AbstractButton.TextBesideIcon
                }
                ColumnLayout {
                    visible: skippedToggle.visible && skippedToggle.checked
                    spacing: Kirigami.Units.smallSpacing
                    Layout.leftMargin: Kirigami.Units.largeSpacing * 2
                    Layout.fillWidth: true
                    Repeater {
                        model: page.skipped
                        delegate: ColumnLayout {
                            required property var modelData
                            spacing: 0
                            Layout.fillWidth: true
                            UqLabel {
                                text: String(modelData.folder)
                                visible: String(modelData.folder) !== ""
                                font.bold: true
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                            UqLabel {
                                text: String(modelData.reason)
                                wrapMode: Text.Wrap
                                opacity: 0.7
                                Layout.fillWidth: true
                            }
                        }
                    }
                }
            }
        }
        SettingRow {
            settingId: "theme-new"
            label: "New theme from this one"
            highlight: App.highlight
            enabled: page.idle
            Layout.fillWidth: true
            QQC2.Button {
                text: "New theme from this one…"
                icon.name: "list-add"
                onClicked: {
                    page.editorWanted = true;
                    tm.openEditor(page.snap.theme || "", false);
                }
            }
        }
        SettingRow {
            settingId: "theme-import"
            label: "Import a theme"
            highlight: App.highlight
            enabled: page.idle
            Layout.fillWidth: true
            QQC2.CheckBox {
                id: importLookBox
                text: "Also use its window gaps, corners and effects"
            }
            QQC2.Button {
                text: "Import…"
                icon.name: "document-import"
                onClicked: importDialog.open()
            }
        }
        SettingRow {
            settingId: "theme-export"
            label: "Export a theme"
            highlight: App.highlight
            enabled: page.idle
            Layout.fillWidth: true
            UqLabel {
                objectName: "exportsWhich"
                text: page.currentThemeName
                elide: Text.ElideRight
                opacity: 0.7
                Layout.maximumWidth: Kirigami.Units.gridUnit * 10
            }
            QQC2.CheckBox {
                id: exportLookBox
                text: "Include window gaps, corners and effects"
            }
            QQC2.Button {
                text: "Export…"
                icon.name: "document-export"
                onClicked: {
                    exportDialog.themeId = page.snap.theme || "";
                    exportDialog.open();
                }
            }
        }
        ProfileMessage {
            id: themeNotice
            objectName: "themeNotice"
            Layout.fillWidth: true
        }
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
                        text: "This theme also has window gaps, corners and effects. Import it again with the box ticked to use them."
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                }
                QQC2.Button {
                    objectName: "useItNow"
                    text: "Use it now"
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
                    Layout.alignment: Qt.AlignTop
                    onClicked: addedCard.dismissed = true
                }
            }
        }
        SettingRow {
            settingId: "variant"
            label: "Light or dark"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
            Repeater {
                model: [{ id: "light", name: "Light" }, { id: "dark", name: "Dark" }]
                QQC2.Button {
                    required property var modelData
                    readonly property bool current: modelData.id === page.snap.variant
                    checkable: true
                    highlighted: current
                    checked: current
                    Accessible.name: modelData.name
                    contentItem: UqLabel {
                        text: modelData.name
                        horizontalAlignment: Text.AlignHCenter
                        font.bold: current
                        color: current ? Kirigami.Theme.highlightColor : Kirigami.Theme.textColor
                        padding: Kirigami.Units.smallSpacing
                    }
                    onClicked: {
                        am.setTheme(page.snap.theme || "", modelData.id);
                        checked = Qt.binding(() => current);
                    }
                }
            }
        }
        ScheduleEditor {
            schedule: page.schedule
            highlight: App.highlight
            enabled: page.ready
            onApply: json => am.setSchedule(json)
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Fonts"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        OverrideCombo {
            settingId: "ui-font"
            source: page.sourceOf("ui-font")
            onReapply: am.reapply("appearance")
            label: "Interface font"
            highlight: App.highlight
            enabled: page.ready
            choices: Qt.fontFamilies()
            value: page.appearance.ui_font || null
            onChosen: v => page.field("ui_font", v)
            Layout.fillWidth: true
        }
        OverrideCombo {
            settingId: "mono-font"
            source: page.sourceOf("mono-font")
            onReapply: am.reapply("appearance")
            label: "Monospace font"
            highlight: App.highlight
            enabled: page.ready
            choices: Qt.fontFamilies()
            value: page.appearance.mono_font || null
            onChosen: v => page.field("mono_font", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: am.busy
            settingId: "font-size"
            source: page.sourceOf("font-size")
            onReapply: am.reapply("appearance")
            label: "Font size"
            highlight: App.highlight
            enabled: page.ready
            from: 6
            to: 32
            fallback: (page.tokens.fonts && page.tokens.fonts.size) || 11
            unit: "pt"
            value: page.appearance.font_size === undefined ? null : page.appearance.font_size
            onChosen: v => page.field("font_size", v)
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Cursor and icons"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        OverrideCombo {
            settingId: "cursor-theme"
            source: page.sourceOf("cursor-theme")
            onReapply: am.reapply("appearance")
            label: "Cursor theme"
            highlight: App.highlight
            enabled: page.ready
            choices: page.snap.cursorThemes || []
            value: page.appearance.cursor_theme || null
            onChosen: v => page.field("cursor_theme", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: am.busy
            settingId: "cursor-size"
            source: page.sourceOf("cursor-size")
            onReapply: am.reapply("appearance")
            label: "Cursor size"
            highlight: App.highlight
            enabled: page.ready
            from: 16
            to: 96
            fallback: (page.tokens.icons && page.tokens.icons.cursor_size) || 24
            unit: "px"
            value: page.appearance.cursor_size === undefined ? null : page.appearance.cursor_size
            onChosen: v => page.field("cursor_size", v)
            Layout.fillWidth: true
        }
        OverrideCombo {
            settingId: "icon-theme"
            source: page.sourceOf("icon-theme")
            onReapply: am.reapply("appearance")
            label: "Icon theme"
            highlight: App.highlight
            enabled: page.ready
            choices: page.snap.iconThemes || []
            value: page.appearance.icon_theme || null
            onChosen: v => page.field("icon_theme", v)
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Qt apps"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SettingRow {
            settingId: "kvantum"
            label: "Style Qt apps to match the theme"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
            QQC2.Switch {
                id: kv
                checked: page.appearance.kvantum !== false
                Accessible.name: "Style Qt apps to match the theme"
                onToggled: {
                    page.field("kvantum", kv.checked);
                    kv.checked = Qt.binding(() => page.appearance.kvantum !== false);
                }
            }
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
