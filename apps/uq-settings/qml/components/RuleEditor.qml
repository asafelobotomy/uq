// The editor for one rule: a window rule (what to match, what to do), a panel rule or a
// workspace rule. Every field comes from `fields`, the catalogue in the page's snapshot (built
// in Rust from uQ's one table of Hyprland names), so this file never lists them; the rule JSON is
// built from the catalogue's `key`s. While a window rule is edited, "Matches now" shows the open
// windows it matches (RulesModel.preview, 150 ms after the last change). Save checks the draft
// first (RulesModel.check) and then stays open until uQ answers: it closes when the rule was
// saved, and shows uQ's refusal otherwise, so a refused rule never loses what was typed. The
// title is a fixed phrase, never the rule's name: a dialog title is drawn by a styled label.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

Kirigami.Dialog {
    id: editor

    /// The snapshot's catalogue: `{match, layer_match, window_effects, layer_effects, workspace}`.
    property var fields: ({})
    /// `text => pattern` that matches exactly that text (RulesModel.exact; tests pass a stub).
    property var exact: text => text
    /// `draftJson => {valid, error, matches}` as JSON (RulesModel.preview), or null for none.
    property var preview: null
    /// `(kind, draftJson, replaces) => {valid, problems}` as JSON (RulesModel.check), or null.
    property var check: null
    property int nameLimit: 100
    property int patternLimit: 256
    /// Whether Save can be pressed (the page isn't busy).
    property bool canSave: true
    /// The rule has a name (or, for a workspace rule, a workspace).
    readonly property bool complete: kind === "workspace" ? selectorBox.text.trim() !== "" : nameBox.text.trim() !== ""

    /// "window", "layer" or "workspace".
    property string kind: "window"
    property bool isNew: true
    /// The rule's id ("" for a new one; workspace rules have none).
    property string ruleId
    /// The saved rule being edited (`{}` for a new one).
    property var original: ({})
    /// `{match: {key: bool}, effect: {key: bool}}`: which fields are included.
    property var ticked: ({ match: {}, effect: {} })
    /// `{match: {key: value}, effect: {key: value}}`: their values, as in the rule JSON.
    property var fieldValues: ({ match: {}, effect: {} })
    /// "Quietly": open on the workspace without switching to it.
    property bool quietly: false
    property var previewResult: ({ valid: true, error: "", matches: [] })
    /// Save was pressed and uQ hasn't answered yet (the page calls finishSave()).
    property bool saving: false
    /// Why the last Save didn't happen, as plain text ("" for none).
    property string saveError
    /// What a save replaces: "" for a new rule, else the id (or workspace) it was opened on.
    readonly property string replaces: isNew ? "" : kind === "workspace" ? String(original.workspace || "") : ruleId
    /// The preview found the window rule can't be saved.
    readonly property bool previewSaysNo: kind === "window" && previewResult.valid === false
    /// Whether Save can be pressed now.
    readonly property bool saveEnabled: canSave && complete && !saving && !previewSaysNo

    // The few rule keys this file has to name, because the catalogue says nothing about them:
    // the pairs of effects that can't both be on, the effect "Quietly" belongs to, and the field
    // a picked window fills. Every other key comes from `fields`.
    readonly property var exclusivePairs: [["float", "tile"], ["move", "center"]]
    readonly property string quietlyBeside: "workspace"
    readonly property string pickFills: "class"

    readonly property var matchFields: (kind === "window" ? fields.match : kind === "layer" ? fields.layer_match : []) || []
    readonly property var effectFields: (kind === "window" ? fields.window_effects : kind === "layer" ? fields.layer_effects : fields.workspace) || []
    readonly property alias nameField: nameBox
    readonly property alias selectorField: selectorBox
    readonly property alias matchList: matchView
    readonly property alias body: body

    /// Save was pressed: `ruleJson` is the rule for RulesModel.save(kind, ruleJson, replaces).
    /// The editor stays open until the page calls finishSave().
    signal ruleAccepted(string kind, string ruleJson, string replaces)
    /// "Pick a window" was pressed: the page picks one and calls fillFromPick().
    signal pickRequested()

    function isOn(group, key) {
        return editor.ticked[group][key] === true;
    }
    function valueOf(group, key) {
        return editor.fieldValues[group][key];
    }
    /// The value a field starts with when it is ticked.
    function startValue(f) {
        const clamp = v => Math.min(f.max === null ? v : f.max, Math.max(f.min === null ? v : f.min, v));
        switch (f.kind) {
        case "bool":
            return true;
        case "int":
            return clamp(0);
        case "float":
            return (f.max === null ? 100 : f.max) / 100;
        case "pair":
            return f.min !== null && f.min >= 1 ? [clamp(800), clamp(600)] : [clamp(0), clamp(0)];
        case "choice":
            return f.choices.length > 0 ? f.choices[0] : "";
        default:
            return "";
        }
    }
    /// Include (or leave out) a field; `value` undefined keeps its value, or starts it.
    function setField(group, key, include, value) {
        const fs = group === "match" ? editor.matchFields : editor.effectFields;
        const f = fs.find(x => x.key === key);
        if (!f)
            return;
        const on = { match: Object.assign({}, editor.ticked.match), effect: Object.assign({}, editor.ticked.effect) };
        const values = { match: Object.assign({}, editor.fieldValues.match), effect: Object.assign({}, editor.fieldValues.effect) };
        on[group][key] = include;
        if (value !== undefined)
            values[group][key] = value;
        else if (values[group][key] === undefined)
            values[group][key] = editor.startValue(f);
        if (include && group === "effect" && editor.kind === "window") {
            for (const pair of editor.exclusivePairs) {
                const i = pair.indexOf(key);
                if (i >= 0)
                    on.effect[pair[1 - i]] = false;
            }
        }
        editor.ticked = on;
        editor.fieldValues = values;
        editor.refreshPreview();
    }
    /// "Use exactly this text": the pattern box's text becomes a pattern for exactly that text.
    function useExact(key) {
        const text = String(editor.valueOf("match", key) || "");
        if (text !== "")
            editor.setField("match", key, true, String(editor.exact(text)));
    }
    /// A window was picked: match it by `classPattern` (from the snapshot's pick).
    function fillFromPick(classPattern) {
        if (classPattern !== "")
            editor.setField("match", editor.pickFills, true, classPattern);
    }
    /// The rule as JSON would have it.
    function draft() {
        const pick = (fs, group) => {
            const out = {};
            for (const f of fs) {
                if (editor.isOn(group, f.key))
                    out[f.key] = editor.valueOf(group, f.key);
            }
            return out;
        };
        if (editor.kind === "workspace")
            return Object.assign({ workspace: selectorBox.text.trim() }, pick(editor.effectFields, "effect"));
        const effects = pick(editor.effectFields, "effect");
        if (editor.kind === "window" && editor.quietly && effects[editor.quietlyBeside] !== undefined)
            effects.silent = true;
        const rule = {
            name: nameBox.text.trim().slice(0, editor.nameLimit),
            enabled: editor.original.enabled !== false,
            match: pick(editor.matchFields, "match"),
            effects: effects
        };
        if (editor.ruleId !== "")
            rule.id = editor.ruleId;
        return rule;
    }
    function ruleJson() {
        return JSON.stringify(editor.draft());
    }
    /// Something changed: ask for "Matches now" again, once typing stops.
    function refreshPreview() {
        if (editor.kind === "window" && typeof editor.preview === "function")
            previewTimer.restart();
    }
    function runPreview() {
        if (typeof editor.preview !== "function")
            return;
        try {
            editor.previewResult = JSON.parse(editor.preview(editor.ruleJson()));
        } catch (e) {
            editor.previewResult = { valid: true, error: "", matches: [] };
        }
    }
    /// Fill the editor with `raw` (a saved rule) or, for null, a blank rule of `kind`.
    function load(kind, raw) {
        const r = raw || {};
        editor.kind = kind;
        editor.isNew = raw === null || raw === undefined;
        editor.original = r;
        editor.ruleId = kind === "workspace" ? "" : String(r.id || "");
        const on = { match: {}, effect: {} };
        const values = { match: {}, effect: {} };
        const take = (fs, from, group) => {
            for (const f of fs) {
                // An effect has no Yes or No: one stored as false does nothing, so it starts
                // unticked (and is left out when saved).
                if (group === "effect" && f.kind === "bool" && from && from[f.key] === false)
                    continue;
                if (from && from[f.key] !== undefined && from[f.key] !== null) {
                    on[group][f.key] = true;
                    values[group][f.key] = from[f.key];
                }
            }
        };
        take(editor.matchFields, r.match, "match");
        take(editor.effectFields, kind === "workspace" ? r : r.effects, "effect");
        editor.ticked = on;
        editor.fieldValues = values;
        editor.quietly = kind === "window" && !!r.effects && r.effects.silent === true;
        nameBox.text = String(r.name || "");
        selectorBox.text = kind === "workspace" ? String(r.workspace || "") : "";
        editor.previewResult = { valid: true, error: "", matches: [] };
        editor.saving = false;
        editor.saveError = "";
        editor.refreshPreview();
    }
    function startNew(kind) {
        editor.load(kind, null);
        editor.open();
    }
    function edit(kind, raw) {
        editor.load(kind, raw);
        editor.open();
    }
    function save() {
        if (!editor.saveEnabled)
            return;
        const json = editor.ruleJson();
        if (typeof editor.check === "function") {
            let answer = null;
            try {
                answer = JSON.parse(editor.check(editor.kind, json, editor.replaces));
            } catch (e) {
                answer = null;
            }
            if (answer && answer.valid === false) {
                editor.saveError = "This rule can't be saved yet: " + (answer.problems || []).map(p => String(p)).join(" ");
                return;
            }
        }
        editor.saveError = "";
        editor.saving = true;
        editor.ruleAccepted(editor.kind, json, editor.replaces);
    }
    /// uQ answered the save: close on success, else keep the draft and say why.
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

    title: editor.isNew ? "New rule" : "Edit rule"
    standardButtons: Kirigami.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 32
    onOpened: (editor.kind === "workspace" ? selectorBox : nameBox).forceActiveFocus()
    customFooterActions: [
        Kirigami.Action {
            text: "Save"
            icon.name: "document-save"
            enabled: editor.saveEnabled
            onTriggered: editor.save()
        }
    ]

    // One field of the catalogue: a box to include it, and its value. An inline component can't
    // see the dialog's ids, so it takes the dialog as `ed`.
    component FieldRow: ColumnLayout {
        id: row

        required property var modelData
        property var ed
        property string group
        readonly property string key: modelData.key
        readonly property bool included: ed ? ed.isOn(group, key) : false
        readonly property var value: ed ? ed.valueOf(group, key) : undefined
        readonly property string kind: modelData.kind
        readonly property int lo: modelData.min === null ? 0 : modelData.min
        readonly property int hi: modelData.max === null ? 100000 : modelData.max

        /// Put the value into the text box (not a binding: one assignment would break it).
        function syncText() {
            const t = row.value === undefined || row.value === null ? "" : String(row.value);
            if (textBox.text !== t)
                textBox.text = t;
        }

        spacing: 0
        Layout.fillWidth: true
        onValueChanged: row.syncText()
        Component.onCompleted: row.syncText()

        RowLayout {
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true

            QQC2.CheckBox {
                id: box
                objectName: row.group + "On_" + row.key
                checked: row.included
                Accessible.name: row.modelData.label
                onToggled: {
                    row.ed.setField(row.group, row.key, box.checked, undefined);
                    box.checked = Qt.binding(() => row.included);
                }
            }
            UqLabel {
                text: row.modelData.label
                Layout.minimumWidth: Kirigami.Units.gridUnit * 8
                MouseArea {
                    anchors.fill: parent
                    onClicked: row.ed.setField(row.group, row.key, !row.included, undefined)
                }
            }
            // Yes or No, for a thing a window is or isn't (match fields only).
            UqComboBox {
                objectName: row.group + "YesNo_" + row.key
                visible: row.kind === "bool" && row.group === "match"
                enabled: row.included
                model: ["Yes", "No"]
                currentIndex: row.value === false ? 1 : 0
                Accessible.name: row.modelData.label
                onActivated: index => row.ed.setField(row.group, row.key, true, index === 0)
            }
            UqComboBox {
                objectName: row.group + "Choice_" + row.key
                visible: row.kind === "choice"
                enabled: row.included
                model: row.modelData.choices
                currentIndex: Math.max(0, row.modelData.choices.indexOf(row.value))
                Accessible.name: row.modelData.label
                onActivated: index => row.ed.setField(row.group, row.key, true, row.modelData.choices[index])
            }
            QQC2.TextField {
                id: textBox
                objectName: row.group + "Value_" + row.key
                visible: row.kind === "text" || row.kind === "pattern"
                maximumLength: row.ed ? row.ed.patternLimit : 256
                placeholderText: row.kind === "pattern" ? "Pattern" : ""
                Accessible.name: row.modelData.label
                Layout.fillWidth: true
                onTextEdited: row.ed.setField(row.group, row.key, row.included || textBox.text !== "", textBox.text)
            }
            QQC2.Button {
                objectName: row.group + "Exact_" + row.key
                visible: row.kind === "pattern"
                text: "Use exactly this text"
                Accessible.description: "Match exactly what is typed, not as a pattern"
                onClicked: row.ed.useExact(row.key)
            }
            QQC2.SpinBox {
                objectName: row.group + "Number_" + row.key
                visible: row.kind === "int" || row.kind === "float"
                editable: true
                from: row.lo
                to: row.hi
                stepSize: row.kind === "float" ? 5 : 1
                value: row.value === undefined ? row.lo : (row.kind === "float" ? Math.round(row.value * 100) : row.value)
                textFromValue: (v, locale) => row.kind === "float" ? (v / 100).toFixed(2) : String(v)
                valueFromText: (text, locale) => row.kind === "float" ? Math.round(parseFloat(text) * 100) : parseInt(text, 10)
                Accessible.name: row.modelData.label
                onValueModified: row.ed.setField(row.group, row.key, true, row.kind === "float" ? value / 100 : value)
            }
            QQC2.SpinBox {
                id: first
                objectName: row.group + "A_" + row.key
                visible: row.kind === "pair"
                editable: true
                from: row.lo
                to: row.hi
                value: Array.isArray(row.value) ? row.value[0] : row.lo
                Accessible.name: row.modelData.label + ", first number"
                onValueModified: row.ed.setField(row.group, row.key, true, [value, second.value])
            }
            QQC2.SpinBox {
                id: second
                objectName: row.group + "B_" + row.key
                visible: row.kind === "pair"
                editable: true
                from: row.lo
                to: row.hi
                value: Array.isArray(row.value) ? row.value[1] : row.lo
                Accessible.name: row.modelData.label + ", second number"
                onValueModified: row.ed.setField(row.group, row.key, true, [first.value, value])
            }
            QQC2.CheckBox {
                objectName: row.group === "effect" && !!row.ed && row.key === row.ed.quietlyBeside ? "quietly" : ""
                visible: !!row.ed && row.group === "effect" && row.ed.kind === "window" && row.key === row.ed.quietlyBeside
                enabled: row.included
                text: "Quietly"
                checked: !!row.ed && row.ed.quietly
                Accessible.description: "Open it there without switching to that workspace"
                onToggled: {
                    row.ed.quietly = checked;
                    row.ed.refreshPreview();
                }
            }
        }
        UqLabel {
            visible: row.group === "effect" && row.modelData.static === true
            text: "applies when the window opens"
            opacity: 0.7
            font.italic: true
            Layout.leftMargin: Kirigami.Units.gridUnit * 2
        }
    }

    // The dialog's one child (Kirigami.Dialog's mainItem), so the timer lives in here too.
    ColumnLayout {
        id: body
        spacing: Kirigami.Units.smallSpacing

        Timer {
            id: previewTimer
            interval: 150
            onTriggered: editor.runPreview()
        }

        UqLabel {
            text: "Name"
            visible: editor.kind !== "workspace"
        }
        QQC2.TextField {
            id: nameBox
            objectName: "ruleName"
            visible: editor.kind !== "workspace"
            maximumLength: editor.nameLimit
            placeholderText: "Such as Volume control floats"
            Accessible.name: "Name of the rule"
            Layout.fillWidth: true
            onTextEdited: editor.refreshPreview()
            onAccepted: editor.save()
        }
        UqLabel {
            text: "Workspace"
            visible: editor.kind === "workspace"
        }
        QQC2.TextField {
            id: selectorBox
            objectName: "selector"
            visible: editor.kind === "workspace"
            // A workspace rule is known by its workspace: editing one keeps it.
            readOnly: !editor.isNew
            maximumLength: 64
            placeholderText: "A number, or name:web"
            Accessible.name: "Which workspace"
            Layout.fillWidth: true
        }

        RowLayout {
            visible: editor.matchFields.length > 0
            Layout.topMargin: Kirigami.Units.largeSpacing
            UqLabel {
                text: "Match"
                font.bold: true
                Accessible.role: Accessible.Heading
                Layout.fillWidth: true
            }
            QQC2.Button {
                objectName: "editorPick"
                visible: editor.kind === "window"
                enabled: editor.canSave && !editor.saving
                text: "Pick a window"
                icon.name: "edit-select"
                onClicked: editor.pickRequested()
            }
        }
        Repeater {
            model: editor.matchFields
            delegate: FieldRow {
                ed: editor
                group: "match"
            }
        }

        UqLabel {
            text: editor.kind === "workspace" ? "Settings" : "Effects"
            font.bold: true
            Accessible.role: Accessible.Heading
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        Repeater {
            model: editor.effectFields
            delegate: FieldRow {
                ed: editor
                group: "effect"
            }
        }

        UqLabel {
            objectName: "previewError"
            visible: editor.kind === "window" && editor.previewResult.valid === false && String(editor.previewResult.error || "") !== ""
            text: "This rule can't be saved yet: " + String(editor.previewResult.error || "")
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.Wrap
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
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
        RuleMatchList {
            id: matchView
            visible: editor.kind === "window" && typeof editor.preview === "function"
            matches: editor.previewResult.matches || []
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            visible: matchView.visible
            text: "Preview is a close guide: Hyprland can differ on letters outside English."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }
        AdvancedDetails {
            kind: editor.kind
            ruleId: editor.ruleId
            rows: editor.matchFields.filter(f => editor.isOn("match", f.key)).concat(editor.effectFields.filter(f => editor.isOn("effect", f.key)))
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
    }
}
