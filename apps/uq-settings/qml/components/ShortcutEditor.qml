// The editor for one shortcut. For one of uQ's it changes the keys; for one of your own (or a new
// one) it also sets the name and what it does. The keys come from "Press the keys…" (KeyCapture,
// which pauses uQ's shortcuts while it listens) or from four modifier switches and a key list;
// either way the same value is built. Every change asks `check` (KeybindsModel.check, no request
// to uQ) and the warning under the keys says who already uses them: Save stays off while the
// answer is "no" and comes back when the keys change. Save then stays open until uQ answers,
// like RuleEditor, so a refused shortcut never loses what was typed. Names, app names and the
// warning are plain text. The title is a fixed phrase, never the shortcut's name.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

Kirigami.Dialog {
    id: editor

    /// The snapshot's `catalogue`: `{keys: [{spelling, label, kind}], actions: {shell, window,
    /// workspace, uq: [{label, action, may_lock, may_repeat}]}}`.
    property var catalogue: ({})
    /// The installed apps `[{id, name, icon}]`, for "Start an app".
    property var apps: []
    /// `{name, command}`: how long a name and a command may be.
    property var limits: ({})
    /// `draftJson => {valid, error, conflicts, user_lua, combo_text}` as JSON, or null for none.
    property var check: null
    /// `(key, modifiers) => "SUPER + Q"` (KeybindsModel.comboFromKey), for the capture field.
    property var comboFrom: (key, modifiers) => ""
    /// Whether pausing the shortcuts works; false hides "Press the keys…".
    property bool captureAvailable: true
    /// Whether Save can be pressed (the page isn't busy).
    property bool canSave: true
    /// Whether a request to uQ is in flight. A save ends when it goes false.
    property bool busy: false
    /// `() => text`: the message of the last refused request, "" for none.
    property var errorNow: () => ""

    property bool isNew: true
    /// One of your own shortcuts (name and action), as against one of uQ's (keys only).
    property bool custom: true
    property string bindId
    /// The row being changed (`{}` for a new shortcut).
    property var original: ({})
    /// Which modifiers are on, and the key's spelling ("" for none yet).
    property var modState: ({ SUPER: false, SHIFT: false, CTRL: false, ALT: false })
    property string keyName
    /// The kind of action: "shell", "window", "workspace", "uq", "launch" or "command".
    property string kind: "uq"
    property int actionIndex: 0
    property string appId
    /// Which kinds of key one of uQ's may take: it keeps the kind it has (a mouse button stays a
    /// mouse button).
    property var allowedKinds: ["key", "wheel"]
    property var verdict: ({ valid: true, error: "", conflicts: [], user_lua: [], combo_text: "" })
    property bool saving: false
    property string saveError

    readonly property var kinds: [
        { id: "shell", label: "Open a uQ panel" },
        { id: "window", label: "Window" },
        { id: "workspace", label: "Workspace" },
        { id: "uq", label: "uQ" },
        { id: "launch", label: "Start an app" },
        { id: "command", label: "Run a command" }
    ]
    readonly property var modifierNames: ["SUPER", "SHIFT", "CTRL", "ALT"]
    readonly property var keyChoices: (catalogue.keys || []).filter(k => allowedKinds.indexOf(k.kind) >= 0)
    readonly property var entries: kind === "launch" || kind === "command" ? [] : ((catalogue.actions || {})[kind] || [])
    readonly property var currentEntry: entries[actionIndex] || null
    readonly property bool mayLock: currentEntry !== null && currentEntry.may_lock === true
    readonly property bool mayRepeat: currentEntry !== null && currentEntry.may_repeat === true
    /// The keys, as `SUPER + Q`; "" until a key is chosen.
    readonly property string combo: keyName === "" ? "" : editor.modifierNames.filter(m => editor.modState[m] === true).concat([keyName]).join(" + ")
    readonly property bool actionComplete: kind === "launch" ? appId !== "" : kind === "command" ? commandBox.text.trim() !== "" : currentEntry !== null
    readonly property bool complete: combo !== "" && (!custom || (nameBox.text.trim() !== "" && actionComplete))
    /// Whether Save can be pressed now.
    readonly property bool saveEnabled: canSave && complete && verdict.valid !== false && !saving
    readonly property alias nameField: nameBox
    readonly property alias commandField: commandBox
    readonly property alias capture: keyCapture
    readonly property alias body: body

    /// Save was pressed for one of your own: `bindJson` for KeybindsModel.saveCustom. The editor
    /// stays open until the page calls finishSave().
    signal bindAccepted(string bindJson)
    /// Save was pressed for one of uQ's: KeybindsModel.setCombo(id, combo).
    signal comboAccepted(string id, string combo)
    /// "Press the keys…" began and ended: the page pauses and resumes the shortcuts.
    signal captureStarted()
    signal captureEnded()

    function setFromCombo(text) {
        const parts = String(text || "").split(" + ").filter(p => p !== "");
        const on = { SUPER: false, SHIFT: false, CTRL: false, ALT: false };
        for (const p of parts.slice(0, -1))
            on[p.toUpperCase()] = true;
        editor.modState = on;
        editor.keyName = parts.length > 0 ? parts[parts.length - 1] : "";
    }
    function setModifier(name, on) {
        const next = Object.assign({}, editor.modState);
        next[name] = on;
        editor.modState = next;
    }
    function indexOfKey(spelling) {
        return editor.keyChoices.findIndex(k => String(k.spelling).toLowerCase() === String(spelling).toLowerCase());
    }
    function kindOfKey(spelling) {
        const k = (editor.catalogue.keys || []).find(x => String(x.spelling).toLowerCase() === String(spelling).toLowerCase());
        return k ? k.kind : "key";
    }
    /// The shortcut as JSON would have it, for `check` and for saving.
    function draft() {
        const d = { combo: editor.combo };
        if (editor.bindId !== "")
            d.id = editor.bindId;
        if (editor.custom) {
            d.name = nameBox.text.trim().slice(0, editor.limits.name || 100);
            d.action = editor.actionJson();
            d.enabled = editor.original.enabled !== false;
            d.locked = editor.mayLock && lockedSwitch.checked;
            d.repeating = editor.mayRepeat && repeatSwitch.checked;
        }
        return d;
    }
    function actionJson() {
        if (editor.kind === "launch")
            return { kind: "launch", app: editor.appId };
        if (editor.kind === "command")
            return { kind: "command", command: commandBox.text.trim() };
        return editor.currentEntry !== null ? editor.currentEntry.action : { kind: "uq", name: "terminal" };
    }
    /// Something changed: ask whether these keys can be used.
    function refreshVerdict() {
        if (typeof editor.check !== "function" || editor.combo === "") {
            editor.verdict = { valid: true, error: "", conflicts: [], user_lua: [], combo_text: "" };
            return;
        }
        try {
            editor.verdict = JSON.parse(editor.check(JSON.stringify(editor.draft())));
        } catch (e) {
            editor.verdict = { valid: true, error: "", conflicts: [], user_lua: [], combo_text: "" };
        }
    }
    /// Fill the editor with `row` (a row of the snapshot) or, for null, a new shortcut of your own.
    function load(row, custom) {
        const r = row || {};
        const raw = r.raw || {};
        editor.custom = custom;
        editor.isNew = row === null || row === undefined;
        editor.original = custom ? raw : r;
        editor.bindId = editor.isNew ? "" : String(r.id || "");
        editor.saving = false;
        editor.saveError = "";
        keyCapture.stop();
        const own = editor.isNew ? "" : String(custom ? raw.combo || "" : r.combo || "");
        const ownKind = own === "" ? "key" : editor.kindOfKey(own.split(" + ").pop());
        editor.allowedKinds = custom ? ["key", "wheel"] : (ownKind === "mod_key" ? ["mod_key", "key"] : [ownKind]);
        editor.setFromCombo(own);
        const action = raw.action || { kind: "uq", name: "terminal" };
        editor.kind = custom ? String(action.kind || "uq") : "uq";
        editor.appId = editor.kind === "launch" ? String(action.app || "") : "";
        commandBox.text = editor.kind === "command" ? String(action.command || "") : "";
        const at = editor.entries.findIndex(e => JSON.stringify(e.action) === JSON.stringify(action));
        editor.actionIndex = Math.max(0, at);
        nameBox.text = custom ? String(raw.name || "") : "";
        lockedSwitch.checked = raw.locked === true;
        repeatSwitch.checked = raw.repeating === true;
        editor.refreshVerdict();
    }
    function startNew() {
        editor.load(null, true);
        editor.open();
    }
    /// Change a row of the snapshot: your own shortcut in full, one of uQ's by its keys.
    function change(row) {
        editor.load(row, row.name !== undefined && row.name !== null);
        editor.open();
    }
    function stopCapture() {
        keyCapture.stop();
    }
    function save() {
        if (!editor.saveEnabled)
            return;
        editor.refreshVerdict();
        if (editor.verdict.valid === false)
            return;
        editor.saveError = "";
        editor.saving = true;
        if (editor.custom)
            editor.bindAccepted(JSON.stringify(editor.draft()));
        else
            editor.comboAccepted(editor.bindId, editor.combo);
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

    onComboChanged: editor.refreshVerdict()
    onKindChanged: editor.refreshVerdict()
    onActionIndexChanged: editor.refreshVerdict()
    onAppIdChanged: editor.refreshVerdict()
    // A save ends when the request does, whether or not the page's state changed. Later, so the
    // request's error has been set by then.
    onBusyChanged: {
        if (!editor.busy && editor.saving)
            Qt.callLater(() => editor.finishSave(editor.errorNow()));
    }
    onClosed: keyCapture.stop()

    title: editor.isNew ? "New shortcut" : "Change shortcut"
    standardButtons: Kirigami.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 30
    customFooterActions: [
        Kirigami.Action {
            text: "Save"
            icon.name: "document-save"
            enabled: editor.saveEnabled
            onTriggered: editor.save()
        }
    ]

    ColumnLayout {
        id: body
        spacing: Kirigami.Units.smallSpacing

        UqLabel {
            text: "Name"
            visible: editor.custom
        }
        QQC2.TextField {
            id: nameBox
            objectName: "shortcutName"
            visible: editor.custom
            maximumLength: editor.limits.name || 100
            placeholderText: "Such as Open my terminal"
            Accessible.name: "Name of the shortcut"
            Layout.fillWidth: true
            onTextEdited: editor.refreshVerdict()
        }

        UqLabel {
            text: "Keys"
            font.bold: true
            Accessible.role: Accessible.Heading
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            objectName: "comboWords"
            text: editor.combo === "" ? "No keys chosen yet." : String(editor.verdict.combo_text || editor.combo)
            opacity: editor.combo === "" ? 0.7 : 1
            font.bold: editor.combo !== ""
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        KeyCapture {
            id: keyCapture
            objectName: "keyCapture"
            available: editor.captureAvailable && editor.allowedKinds.indexOf("key") >= 0
            canStart: editor.canSave && !editor.saving
            comboFrom: (key, modifiers) => editor.comboFrom(key, modifiers)
            onCaptured: combo => editor.setFromCombo(combo)
            onStarted: editor.captureStarted()
            onEnded: editor.captureEnded()
        }
        UqLabel {
            text: "Or choose them"
            opacity: 0.7
        }
        RowLayout {
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true

            QQC2.Switch {
                objectName: "mod_SUPER"
                text: "Super"
                checked: editor.modState.SUPER === true
                onToggled: {
                    editor.setModifier("SUPER", checked);
                    checked = Qt.binding(() => editor.modState.SUPER === true);
                }
            }
            QQC2.Switch {
                objectName: "mod_SHIFT"
                text: "Shift"
                checked: editor.modState.SHIFT === true
                onToggled: {
                    editor.setModifier("SHIFT", checked);
                    checked = Qt.binding(() => editor.modState.SHIFT === true);
                }
            }
            QQC2.Switch {
                objectName: "mod_CTRL"
                text: "Ctrl"
                checked: editor.modState.CTRL === true
                onToggled: {
                    editor.setModifier("CTRL", checked);
                    checked = Qt.binding(() => editor.modState.CTRL === true);
                }
            }
            QQC2.Switch {
                objectName: "mod_ALT"
                text: "Alt"
                checked: editor.modState.ALT === true
                onToggled: {
                    editor.setModifier("ALT", checked);
                    checked = Qt.binding(() => editor.modState.ALT === true);
                }
            }
        }
        UqComboBox {
            id: keyBox
            objectName: "keyPicker"
            model: editor.keyChoices.map(k => String(k.label))
            currentIndex: editor.indexOfKey(editor.keyName)
            Accessible.name: "Key"
            Layout.fillWidth: true
            onActivated: index => editor.keyName = String(editor.keyChoices[index].spelling)
        }

        // The warning: who already uses these keys, as the same check uQ makes on a write.
        UqLabel {
            objectName: "conflictWarning"
            visible: editor.combo !== "" && editor.verdict.valid === false && String(editor.verdict.error || "") !== ""
            text: String(editor.verdict.error || "")
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "userLuaWarning"
            visible: editor.combo !== "" && (editor.verdict.user_lua || []).length > 0
            text: "A shortcut in user.lua, line " + ((editor.verdict.user_lua || [{ line: 0 }])[0].line) + ", uses these keys too."
            wrapMode: Text.Wrap
            opacity: 0.8
            Layout.fillWidth: true
        }

        UqLabel {
            text: "What it does"
            font.bold: true
            visible: editor.custom
            Accessible.role: Accessible.Heading
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        UqComboBox {
            id: kindBox
            objectName: "actionKind"
            visible: editor.custom
            model: editor.kinds.map(k => k.label)
            currentIndex: Math.max(0, editor.kinds.findIndex(k => k.id === editor.kind))
            Accessible.name: "Kind of action"
            Layout.fillWidth: true
            onActivated: index => {
                editor.kind = editor.kinds[index].id;
                editor.actionIndex = 0;
            }
        }
        UqComboBox {
            id: actionBox
            objectName: "actionChoice"
            visible: editor.custom && editor.entries.length > 0
            model: editor.entries.map(e => String(e.label))
            currentIndex: editor.actionIndex
            Accessible.name: "Action"
            Layout.fillWidth: true
            onActivated: index => editor.actionIndex = index
        }
        UqComboBox {
            id: appBox
            objectName: "appChoice"
            visible: editor.custom && editor.kind === "launch"
            model: editor.apps.map(a => String(a.name))
            currentIndex: editor.apps.findIndex(a => a.id === editor.appId)
            Accessible.name: "App to start"
            Layout.fillWidth: true
            onActivated: index => editor.appId = String(editor.apps[index].id)
        }
        QQC2.TextField {
            id: commandBox
            objectName: "commandText"
            visible: editor.custom && editor.kind === "command"
            maximumLength: editor.limits.command || 1024
            placeholderText: "Such as foot --title notes"
            Accessible.name: "Command to run"
            Layout.fillWidth: true
            onTextEdited: editor.refreshVerdict()
        }
        UqLabel {
            visible: editor.custom && editor.kind === "command"
            text: "It runs on its own, so it keeps going if uQ restarts."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }
        QQC2.Switch {
            id: lockedSwitch
            objectName: "lockedSwitch"
            visible: editor.custom && editor.mayLock
            text: "Also over the lock screen"
            onToggled: editor.refreshVerdict()
        }
        QQC2.Switch {
            id: repeatSwitch
            objectName: "repeatSwitch"
            visible: editor.custom && editor.mayRepeat
            text: "Repeat while held"
            onToggled: editor.refreshVerdict()
        }

        UqLabel {
            objectName: "saveError"
            visible: editor.saveError !== ""
            text: editor.saveError
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.Wrap
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
    }
}
