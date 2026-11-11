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
    readonly property var platforms: dialog.fields.platforms || []
    readonly property var scales: dialog.fields.scales || []
    readonly property var limits: dialog.fields.limits || ({})
    readonly property var scaleLabels: ["Automatic"].concat(dialog.scales.map(s => Math.round(s * 100) + " %"))
    readonly property string problem: dialog.previewResult && dialog.previewResult.problem ? String(dialog.previewResult.problem) : ""
    readonly property bool saveEnabled: canSave && appId !== "" && !saving && problem === ""
    readonly property alias body: body
    readonly property alias platformBox: platformBox
    readonly property alias scaleBox: scaleBox
    readonly property alias argsBox: argsBox
    readonly property alias elsewhereBox: elsewhereBox

    /// Save was pressed: `fixJson` is the fix for CompatModel.saveFix. The dialog stays open
    /// until the page calls finishSave(). (Not `accepted`: a Dialog has that signal already.)
    signal fixAccepted(string fixJson)

    /// Open on `fix` (a `fixes` row, or null for a new one) for app `id` named `name`.
    function start(id, name, fix) {
        const f = fix || {};
        dialog.appId = String(id || "");
        dialog.appName = String(name || "");
        dialog.choosing = false;
        dialog.windowText = "";
        platformBox.currentIndex = Math.max(0, dialog.platforms.findIndex(p => p.key === (f.platform || "auto")));
        const s = dialog.scales.findIndex(x => Math.abs(x - Number(f.scale)) < 0.001);
        scaleBox.currentIndex = f.scale === null || f.scale === undefined || s < 0 ? 0 : s + 1;
        dialog.envList = (f.env || []).map(v => ({ name: String(v.name), value: String(v.value) }));
        argsBox.text = (f.args || []).map(a => String(a)).join("\n");
        elsewhereBox.checked = f.elsewhere === true;
        dialog.saving = false;
        dialog.saveError = "";
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
    /// The fix as the `compat` section holds it.
    function draft() {
        const p = dialog.platforms[platformBox.currentIndex];
        const args = argsBox.text.split("\n").map(a => a.trim()).filter(a => a !== "");
        return {
            id: dialog.appId,
            platform: p ? p.key : "auto",
            scale: scaleBox.currentIndex > 0 ? dialog.scales[scaleBox.currentIndex - 1] : null,
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
        if (typeof dialog.preview !== "function")
            return;
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
        dialog.refreshPreview();
    }
    function save() {
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
            model: dialog.apps.map(a => String(a.name))
            Accessible.name: "App"
            Layout.fillWidth: true
            onActivated: index => {
                const a = dialog.apps[index];
                dialog.appId = a ? String(a.id) : "";
                dialog.appName = a ? String(a.name) : "";
                dialog.refreshPreview();
            }
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
            onActivated: dialog.refreshPreview()
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
            onActivated: dialog.refreshPreview()
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
                    dialog.refreshPreview();
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
            onTextChanged: dialog.refreshPreview()
        }

        QQC2.CheckBox {
            id: elsewhereBox
            objectName: "elsewhere"
            text: "Also when started from other launchers"
            Layout.topMargin: Kirigami.Units.largeSpacing
            onToggled: dialog.refreshPreview()
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
            visible: dialog.problem !== ""
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
            problem: dialog.problem
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
    }
}
