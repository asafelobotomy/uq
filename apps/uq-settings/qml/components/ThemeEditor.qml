// The theme editor: a new theme made from another one, or one of the user's own themes. A name,
// an optional author, and seven colours for the dark version and for the light version (each a
// swatch that opens a colour chooser, and a hex box). Everything else in the palette is worked
// out from those, and the live preview below is painted from what ThemesModel.preview says, so
// this file knows no colour rules. Problems (text that would be hard to read) show as plain lines
// and keep Save off. The colour rows come from the session's `keys`, and the draft is built from
// those keys only. Save stays open until uQ answers: it closes when the theme was saved and shows
// uQ's refusal otherwise, so a refused theme never loses what was typed. The title is one of two
// fixed phrases, never the theme's name: a dialog title is drawn by a styled label.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

Kirigami.Dialog {
    id: editor

    /// `draftJson => {dark, light, ok, nameProblem, authorProblem}` as JSON (ThemesModel.preview),
    /// or null for none.
    property var preview: null
    /// Whether Save can be pressed (the page isn't busy).
    property bool canSave: true
    /// Whether a request to uQ is in flight (ThemesModel.busy). A save ends when it goes false.
    property bool busy: false
    /// `() => text`: the message of the last refused request, "" for none (ThemesModel.error).
    property var errorNow: () => ""
    /// A new theme (not an edit of one of the user's).
    property bool isNew: true
    /// The colour rows: `[{key, label}]`, from the session.
    property var keys: []
    /// The colours as typed: `{dark: {key: text}, light: {key: text}}`.
    property var colors: ({ dark: {}, light: {} })
    /// "dark" or "light": the tab that is open, and the one the preview shows.
    property string variant: "dark"
    property var previewResult: ({
        dark: { tokens: {}, problems: [] },
        light: { tokens: {}, problems: [] },
        ok: false,
        nameProblem: "",
        authorProblem: ""
    })
    /// Save was pressed and uQ hasn't answered yet (finishSave() ends it).
    property bool saving: false
    /// Why the last Save didn't happen, as plain text ("" for none).
    property string saveError
    readonly property var shown: previewResult[variant] || { tokens: {}, problems: [] }
    readonly property var otherShown: previewResult[variant === "dark" ? "light" : "dark"] || { problems: [] }
    readonly property bool saveEnabled: canSave && !saving && previewResult.ok === true
    readonly property alias body: body
    readonly property alias nameField: nameBox
    readonly property alias authorField: authorBox

    /// Save was pressed with this draft: `{name, author, dark: {key: hex}, light: {…}}`.
    signal saved(string draftJson)

    /// Open on a session: `{edit, name, author, keys, colors}` (the Themes page state's `editor`).
    function start(session) {
        editor.isNew = session.edit !== true;
        editor.keys = session.keys || [];
        editor.colors = {
            dark: Object.assign({}, (session.colors || {}).dark || {}),
            light: Object.assign({}, (session.colors || {}).light || {})
        };
        nameBox.text = String(session.name || "");
        authorBox.text = String(session.author || "");
        editor.variant = "dark";
        tabs.currentIndex = 0;
        editor.saving = false;
        editor.saveError = "";
        editor.runPreview();
        editor.open();
    }
    /// The draft as JSON: the name, the author and the colour of each key.
    function draftJson() {
        const pick = v => {
            const out = {};
            for (const k of editor.keys)
                out[k.key] = String(editor.colors[v][k.key] ?? "");
            return out;
        };
        return JSON.stringify({
            name: nameBox.text,
            author: authorBox.text,
            dark: pick("dark"),
            light: pick("light")
        });
    }
    /// A colour box (or the chooser) set one colour. The text goes on as typed: uQ reads it.
    function setColor(variant, key, text) {
        const c = { dark: Object.assign({}, editor.colors.dark), light: Object.assign({}, editor.colors.light) };
        c[variant][key] = text;
        editor.colors = c;
        editor.runPreview();
    }
    function runPreview() {
        if (typeof editor.preview !== "function")
            return;
        try {
            editor.previewResult = JSON.parse(editor.preview(editor.draftJson()));
        } catch (e) {
            // Unreadable: nothing can be saved.
            editor.previewResult = Object.assign({}, editor.previewResult, { ok: false });
        }
    }
    /// A colour from the chooser as `#rrggbb`.
    function hexOf(c) {
        const part = v => Math.round(v * 255).toString(16).padStart(2, "0");
        return "#" + part(c.r) + part(c.g) + part(c.b);
    }
    function save() {
        if (!editor.saveEnabled)
            return;
        editor.saveError = "";
        editor.saving = true;
        editor.saved(editor.draftJson());
    }
    /// uQ answered the save: close on success, else keep the draft and say why. Only the first
    /// call after a Save does anything, so the page may call it from more than one place.
    function finishSave(error) {
        if (!editor.saving)
            return;
        editor.saving = false;
        const why = String(error || "");
        if (why === "")
            editor.close();
        else
            editor.saveError = "That wasn't saved: " + why;
    }

    // A save ends when the request does. Later, so the request's error has been set by then.
    onBusyChanged: {
        if (!editor.busy && editor.saving)
            Qt.callLater(() => editor.finishSave(editor.errorNow()));
    }

    title: editor.isNew ? "New theme" : "Edit theme"
    standardButtons: Kirigami.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 30
    onOpened: nameBox.forceActiveFocus()
    customFooterActions: [
        Kirigami.Action {
            text: "Save"
            icon.name: "document-save"
            enabled: editor.saveEnabled
            onTriggered: editor.save()
        }
    ]

    // The dialog's one child (Kirigami.Dialog's mainItem).
    ColumnLayout {
        id: body
        spacing: Kirigami.Units.smallSpacing

        // Not an Item, so it lives in the dialog's one child.
        ColorDialog {
            id: picker
            property string variantId
            property string keyId
            title: "Choose a colour"
            onAccepted: editor.setColor(picker.variantId, picker.keyId, editor.hexOf(picker.selectedColor))
        }

        UqLabel {
            text: "Name"
        }
        QQC2.TextField {
            id: nameBox
            objectName: "themeNameField"
            maximumLength: 64
            placeholderText: "Such as Ocean at night"
            Accessible.name: "Name of the theme"
            Layout.fillWidth: true
            onTextEdited: editor.runPreview()
            onAccepted: editor.save()
        }
        UqLabel {
            objectName: "nameProblem"
            visible: String(editor.previewResult.nameProblem || "") !== ""
            text: String(editor.previewResult.nameProblem || "")
            wrapMode: Text.Wrap
            color: Kirigami.Theme.negativeTextColor
            Layout.fillWidth: true
        }
        UqLabel {
            text: "Author (optional)"
        }
        QQC2.TextField {
            id: authorBox
            objectName: "themeAuthorField"
            maximumLength: 64
            Accessible.name: "Author of the theme"
            Layout.fillWidth: true
            onTextEdited: editor.runPreview()
        }
        UqLabel {
            objectName: "authorProblem"
            visible: String(editor.previewResult.authorProblem || "") !== ""
            text: String(editor.previewResult.authorProblem || "")
            wrapMode: Text.Wrap
            color: Kirigami.Theme.negativeTextColor
            Layout.fillWidth: true
        }

        QQC2.TabBar {
            id: tabs
            objectName: "variantTabs"
            Layout.topMargin: Kirigami.Units.smallSpacing
            Layout.fillWidth: true
            QQC2.TabButton {
                text: "Dark"
            }
            QQC2.TabButton {
                text: "Light"
            }
            onCurrentIndexChanged: editor.variant = currentIndex === 0 ? "dark" : "light"
        }

        Repeater {
            model: ["dark", "light"]
            delegate: ColumnLayout {
                id: side
                required property string modelData
                visible: editor.variant === side.modelData
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true

                Repeater {
                    model: editor.keys
                    delegate: RowLayout {
                        id: colorRow
                        required property var modelData
                        readonly property string key: String(modelData.key)
                        readonly property string hex: String(editor.colors[side.modelData][key] ?? "")
                        readonly property bool readable: /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(hex.trim())
                        spacing: Kirigami.Units.smallSpacing
                        Layout.fillWidth: true

                        UqLabel {
                            text: String(colorRow.modelData.label)
                            Layout.preferredWidth: Kirigami.Units.gridUnit * 7
                        }
                        QQC2.Button {
                            objectName: "swatch-" + side.modelData + "-" + colorRow.key
                            Accessible.name: String(colorRow.modelData.label) + " colour"
                            implicitWidth: Kirigami.Units.gridUnit * 2.5
                            contentItem: Rectangle {
                                radius: Kirigami.Units.cornerRadius
                                color: colorRow.readable ? colorRow.hex.trim() : "transparent"
                                border.width: 1
                                border.color: Kirigami.Theme.disabledTextColor
                            }
                            onClicked: {
                                picker.variantId = side.modelData;
                                picker.keyId = colorRow.key;
                                if (colorRow.readable)
                                    picker.selectedColor = colorRow.hex.trim();
                                picker.open();
                            }
                        }
                        QQC2.TextField {
                            id: hexBox
                            objectName: side.modelData + "-" + colorRow.key
                            maximumLength: 9
                            font.family: "monospace"
                            Accessible.name: String(colorRow.modelData.label) + " colour as text"
                            Layout.fillWidth: true
                            onTextEdited: editor.setColor(side.modelData, colorRow.key, hexBox.text)
                            // What was typed stays while the box has focus; otherwise it follows
                            // the colours (the chooser, a new session).
                            Binding {
                                target: hexBox
                                property: "text"
                restoreMode: Binding.RestoreNone
                                value: colorRow.hex
                                when: !hexBox.activeFocus
                            }
                        }
                    }
                }
            }
        }

        // What the colours make, painted from the preview's tokens.
        Rectangle {
            id: card
            objectName: "previewCard"
            color: String(editor.shown.tokens.bg || "transparent")
            radius: Kirigami.Units.cornerRadius
            border.width: 1
            border.color: String(editor.shown.tokens.border || "transparent")
            implicitHeight: cardColumn.implicitHeight + Kirigami.Units.largeSpacing * 2
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.smallSpacing
            Accessible.role: Accessible.Graphic
            Accessible.name: "Preview of the " + editor.variant + " version"

            ColumnLayout {
                id: cardColumn
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.smallSpacing

                Rectangle {
                    objectName: "previewPanel"
                    color: String(editor.shown.tokens.surface || "transparent")
                    radius: Kirigami.Units.cornerRadius
                    implicitHeight: panelColumn.implicitHeight + Kirigami.Units.largeSpacing
                    Layout.fillWidth: true

                    ColumnLayout {
                        id: panelColumn
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.smallSpacing
                        spacing: 0
                        UqLabel {
                            objectName: "previewText"
                            text: "Text on a panel"
                            color: String(editor.shown.tokens.text || Kirigami.Theme.textColor)
                            Accessible.ignored: true
                        }
                        UqLabel {
                            objectName: "previewMuted"
                            text: "Quieter text"
                            color: String(editor.shown.tokens.text_muted || Kirigami.Theme.textColor)
                            Accessible.ignored: true
                        }
                    }
                }
                RowLayout {
                    spacing: Kirigami.Units.smallSpacing
                    Rectangle {
                        objectName: "previewAccent"
                        color: String(editor.shown.tokens.accent || "transparent")
                        radius: Kirigami.Units.cornerRadius
                        implicitWidth: accentLabel.implicitWidth + Kirigami.Units.largeSpacing * 2
                        implicitHeight: accentLabel.implicitHeight + Kirigami.Units.smallSpacing * 2
                        UqLabel {
                            id: accentLabel
                            anchors.centerIn: parent
                            text: "Accent"
                            color: String(editor.shown.tokens.accent_text || Kirigami.Theme.textColor)
                            Accessible.ignored: true
                        }
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                    Repeater {
                        model: ["success", "warning", "error"]
                        delegate: Rectangle {
                            required property string modelData
                            objectName: "previewDot-" + modelData
                            implicitWidth: Kirigami.Units.gridUnit
                            implicitHeight: Kirigami.Units.gridUnit
                            radius: width / 2
                            color: String(editor.shown.tokens[modelData] || "transparent")
                        }
                    }
                }
            }
        }

        UqLabel {
            objectName: "problems"
            visible: (editor.shown.problems || []).length > 0
            text: (editor.shown.problems || []).map(p => String(p)).join("\n")
            wrapMode: Text.Wrap
            color: Kirigami.Theme.negativeTextColor
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "otherProblems"
            visible: (editor.otherShown.problems || []).length > 0
            text: "The " + (editor.variant === "dark" ? "light" : "dark") + " version has a problem too."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "saveError"
            visible: editor.saveError !== ""
            text: editor.saveError
            wrapMode: Text.Wrap
            color: Kirigami.Theme.negativeTextColor
            Layout.fillWidth: true
        }
    }
}
