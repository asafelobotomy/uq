// "Fix this app": how one app is started by uQ (spec §6). Run on (Automatic, Wayland or X11),
// Size, extra settings and options for this app, and whether launchers other than uQ's use the
// fix too. Every choice comes from `fields`, the catalogue in the page's snapshot (built in Rust
// from uQ's own tables), so this file never lists them; the fix JSON is built from the
// catalogue's keys. Advanced details shows what the fix adds to the app's command
// (CompatModel.preview). Save stays open until uQ answers, as the rule editor does. The title
// is a fixed phrase, never the app's name: a dialog title is drawn by a styled label.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

Kirigami.Dialog {
    id: dialog

    /// The snapshot's catalogue: `{platforms, scales, limits}`.
    property var fields: ({})
    /// The installed apps `[{id, name, icon}]`, for the chooser.
    property var apps: []
    /// `id => fix` (a `fixes` row) or null: the fix an app already has, so choosing that app
    /// edits it rather than replacing it.
    property var existingFix: id => null
    /// `fixJson => {env, args, problem}` as JSON (CompatModel.preview), or null.
    property var preview: null
    /// Whether Save can be pressed (the page isn't busy).
    property bool canSave: true
    /// Whether a request to uQ is in flight (CompatModel.busy). A save ends when it goes false.
    property bool busy: false
    /// `() => text`: the message of the last refused request (CompatModel.error).
    property var errorNow: () => ""

    /// The app the fix is for ("" until one is chosen).
    property string appId
    property string appName
    /// uQ couldn't tell which app a window is: the person chooses it.
    property bool choosing: false
    /// The window the dialog was opened from (shown while choosing), or "".
    property string windowText
    /// The fix's own variables, `[{name, value}]` (rows edit their entries in place).
    property var envList: []
    /// `{env, args, problem}` for the draft.
    property var previewResult: ({ env: [], args: [], problem: null })
    property bool saving: false
    property string saveError
    /// The draft's "Run on" key and size (null for Automatic). The combo boxes show these; a
    /// new snapshot never changes them.
    property string platformKey: "auto"
    property var scaleValue: null
    /// The catalogue and the app list as they were when the dialog opened: a new snapshot
    /// mustn't rebuild the combo boxes' models, which would put them back to their first entry.
    property var catalogue: ({})
    property var appList: []
    /// The person has changed something since the dialog opened: only then is a problem shown.
    property bool touched: false
    /// The chosen app already had a fix, which the dialog now edits.
    property bool editingExisting: false
    readonly property var platforms: dialog.catalogue.platforms || []
    readonly property var scales: dialog.catalogue.scales || []
    readonly property var limits: dialog.catalogue.limits || ({})
    readonly property var scaleLabels: ["Automatic"].concat(dialog.scales.map(s => Math.round(s * 100) + " %"))
    readonly property string problem: dialog.previewResult && dialog.previewResult.problem ? String(dialog.previewResult.problem) : ""
    readonly property bool saveEnabled: canSave && appId !== "" && !saving && problem === ""
    readonly property int platformIndex: Math.max(0, dialog.platforms.findIndex(p => p.key === dialog.platformKey))
    readonly property int scaleIndex: {
        if (dialog.scaleValue === null || dialog.scaleValue === undefined)
            return 0;
        const i = dialog.scales.findIndex(x => Math.abs(x - Number(dialog.scaleValue)) < 0.001);
        return i < 0 ? 0 : i + 1;
    }
    readonly property alias body: body
    readonly property alias platformBox: platformBox
    readonly property alias scaleBox: scaleBox
    readonly property alias argsBox: argsBox
    readonly property alias elsewhereBox: elsewhereBox

    /// Save was pressed: `fixJson` is the fix for CompatModel.saveFix. The dialog stays open
    /// until the page calls finishSave(). (Not `accepted`: a Dialog has that signal already.)
    signal fixAccepted(string fixJson)

    /// Fill the draft from `fix` (a `fixes` row, or null for a new one).
    function load(fix) {
        const f = fix || {};
        dialog.platformKey = String(f.platform || "auto");
        const s = dialog.scales.findIndex(x => Math.abs(x - Number(f.scale)) < 0.001);
        dialog.scaleValue = f.scale === null || f.scale === undefined || s < 0 ? null : dialog.scales[s];
        dialog.envList = (f.env || []).map(v => ({ name: String(v.name), value: String(v.value) }));
        argsBox.text = (f.args || []).map(a => String(a)).join("\n");
        elsewhereBox.checked = f.elsewhere === true;
        dialog.syncCombos();
    }
    /// Show the draft's "Run on" and size in the combo boxes.
    function syncCombos() {
        platformBox.currentIndex = dialog.platformIndex;
        scaleBox.currentIndex = dialog.scaleIndex;
    }
    /// Open on `fix` (a `fixes` row, or null for a new one) for app `id` named `name`.
    function start(id, name, fix) {
        dialog.catalogue = dialog.fields;
        dialog.appList = dialog.apps;
        dialog.appId = String(id || "");
        dialog.appName = String(name || "");
        dialog.choosing = false;
        dialog.windowText = "";
        dialog.editingExisting = false;
        dialog.load(fix);
        dialog.saving = false;
        dialog.saveError = "";
        dialog.touched = false;
        dialog.runPreview();
        dialog.open();
    }
    /// Open for a window whose app uQ couldn't tell: the person chooses it from the list.
    function startChoosing(windowText) {
        dialog.start("", "", null);
        dialog.choosing = true;
        dialog.windowText = String(windowText || "");
        chooser.currentIndex = -1;
    }
    /// The app at `index` of the chooser was chosen; an app that has a fix already loads it.
    function chooseApp(index) {
        const a = dialog.appList[index];
        dialog.appId = a ? String(a.id) : "";
        dialog.appName = a ? String(a.name) : "";
        const fix = a && typeof dialog.existingFix === "function" ? dialog.existingFix(dialog.appId) : null;
        // Another app's saved fix isn't carried over to this one: start blank instead.
        if (fix || dialog.editingExisting)
            dialog.load(fix);
        dialog.editingExisting = !!fix;
        dialog.markChanged();
    }
    /// The person changed something: check the draft again shortly.
    function markChanged() {
        dialog.touched = true;
        dialog.refreshPreview();
    }
    /// The fix as the `compat` section holds it.
    function draft() {
        const args = argsBox.text.split("\n").map(a => a.trim()).filter(a => a !== "");
        return {
            id: dialog.appId,
            // A catalogue key: set from the catalogue, or kept from the saved fix (F29).
            platform: dialog.platformKey,
            scale: dialog.scaleValue === undefined ? null : dialog.scaleValue,
            env: dialog.envList.filter(v => v.name.trim() !== "" || v.value !== "").map(v => ({ name: v.name.trim(), value: v.value })),
            args: args,
            elsewhere: elsewhereBox.checked
        };
    }
    function fixJson() {
        return JSON.stringify(dialog.draft());
    }
    function refreshPreview() {
        previewTimer.restart();
    }
    function runPreview() {
        // No app yet: nothing to check, and the core's message would name file names.
        if (typeof dialog.preview !== "function" || dialog.appId === "") {
            dialog.previewResult = { env: [], args: [], problem: null };
            return;
        }
        try {
            dialog.previewResult = JSON.parse(dialog.preview(dialog.fixJson()));
        } catch (e) {
            dialog.previewResult = { env: [], args: [], problem: null };
        }
    }
    function addVar() {
        dialog.envList = dialog.envList.concat([{ name: "", value: "" }]);
    }
    function removeVar(index) {
        dialog.envList = dialog.envList.filter((v, i) => i !== index);
        dialog.markChanged();
    }
    function save() {
        // Check the draft as it is now: the preview may lag the last edit (slice C's F17).
        previewTimer.stop();
        dialog.runPreview();
        dialog.touched = true;
        if (!dialog.saveEnabled)
            return;
        dialog.saveError = "";
        dialog.saving = true;
        dialog.fixAccepted(dialog.fixJson());
    }
    /// uQ answered the save: close on success, else keep the draft and say why. Only the first
    /// call after a Save does anything.
    function finishSave(error) {
        if (!dialog.saving)
            return;
        dialog.saving = false;
        const why = String(error || "");
        if (why === "")
            dialog.close();
        else
            dialog.saveError = "That wasn't saved: " + why;
    }

    // A save ends when the request does, whether or not the page's state changed.
    onBusyChanged: {
        if (!dialog.busy && dialog.saving)
            Qt.callLater(() => dialog.finishSave(dialog.errorNow()));
    }

    title: "Fix this app"
    standardButtons: Kirigami.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 32
    customFooterActions: [
        Kirigami.Action {
            text: "Save"
            icon.name: "document-save"
            enabled: dialog.saveEnabled
            onTriggered: dialog.save()
        }
    ]

    // The dialog's one child (Kirigami.Dialog's mainItem), so the timer lives in here too.
    ColumnLayout {
        id: body
        spacing: Kirigami.Units.smallSpacing

        Timer {
            id: previewTimer
            interval: 150
            onTriggered: dialog.runPreview()
        }

        UqLabel {
            objectName: "fixAppName"
            visible: !dialog.choosing
            text: dialog.appName
            font.bold: true
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "chooseHint"
            visible: dialog.choosing
            text: "uQ couldn't tell which app this is. Choose it:"
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            visible: dialog.choosing && dialog.windowText !== ""
            text: dialog.windowText
            elide: Text.ElideRight
            opacity: 0.7
            Layout.fillWidth: true
        }
        UqComboBox {
            id: chooser
            objectName: "appChooser"
            visible: dialog.choosing
            model: dialog.appList.map(a => String(a.name))
            Accessible.name: "App"
            Layout.fillWidth: true
            onActivated: index => dialog.chooseApp(index)
        }
        UqLabel {
            objectName: "existingFix"
            visible: dialog.choosing && dialog.editingExisting
            text: "This app already has a fix. Saving changes it."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Run on"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        UqComboBox {
            id: platformBox
            objectName: "platform"
            model: dialog.platforms.map(p => String(p.label))
            Accessible.name: "Run on"
            onActivated: index => {
                const p = dialog.platforms[index];
                dialog.platformKey = p ? String(p.key) : "auto";
                dialog.markChanged();
            }
            // A new model puts a combo box back on its first entry: show the draft again.
            onModelChanged: Qt.callLater(dialog.syncCombos)
        }
        UqLabel {
            text: "Most blurry apps are sharp with Wayland."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Size"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        UqComboBox {
            id: scaleBox
            objectName: "scale"
            model: dialog.scaleLabels
            Accessible.name: "Size"
            onActivated: index => {
                dialog.scaleValue = index > 0 ? dialog.scales[index - 1] : null;
                dialog.markChanged();
            }
            onModelChanged: Qt.callLater(dialog.syncCombos)
        }

        UqLabel {
            text: "Extra settings for this app"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        Repeater {
            model: dialog.envList
            delegate: EnvVarRow {
                required property var modelData
                required property int index
                name: String(modelData.name)
                value: String(modelData.value)
                showSave: false
                onEdited: (n, v) => {
                    dialog.envList[index] = { name: n, value: v };
                    dialog.markChanged();
                }
                onRemoveRequested: dialog.removeVar(index)
            }
        }
        QQC2.Button {
            objectName: "addFixVar"
            text: "Add a setting"
            icon.name: "list-add"
            enabled: dialog.envList.length < (dialog.limits.env || 20)
            onClicked: dialog.addVar()
        }

        UqLabel {
            text: "Extra options"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            text: "One per line. They are added after the app's own."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }
        QQC2.TextArea {
            id: argsBox
            objectName: "args"
            textFormat: TextEdit.PlainText
            wrapMode: TextEdit.NoWrap
            Accessible.name: "Extra options"
            Layout.fillWidth: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 4
            // Also on load(); start() clears `touched` after it.
            onTextChanged: dialog.markChanged()
        }

        QQC2.CheckBox {
            id: elsewhereBox
            objectName: "elsewhere"
            text: "Also when started from other launchers"
            Layout.topMargin: Kirigami.Units.largeSpacing
            onToggled: dialog.markChanged()
        }
        UqLabel {
            objectName: "elsewhereSentence"
            text: "uQ saves its own copy of this app's launcher file, and removes it when you remove the fix."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.gridUnit * 2
        }

        UqLabel {
            objectName: "fixProblem"
            visible: dialog.touched && dialog.problem !== ""
            text: "This fix can't be saved yet: " + dialog.problem
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.Wrap
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            objectName: "saveError"
            visible: dialog.saveError !== ""
            text: dialog.saveError
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        AdvancedDetails {
            kind: "fix"
            envNames: dialog.previewResult.env || []
            extraArgs: dialog.previewResult.args || []
            problem: dialog.touched ? dialog.problem : ""
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
    }
}
