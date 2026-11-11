// The editor for one touchpad gesture: how many fingers, which way, what it does, a key to hold
// and, for the scratch workspace, its name. The directions offered follow the action (switching
// workspace takes only sideways or up and down). Every change asks `check` (InputModel.draftClash,
// no request to uQ) and a clash is said in a RuleMessage under the choices; Save stays off while
// there is one. Enter saves and Escape cancels. Save leaves the dialog open until the page calls
// finishSave(), so a refused gesture never loses what was chosen (like ShortcutEditor). Every
// label comes from uQ's own tables and is shown through UqComboBox (plain text).
//
// `choices` is the snapshot's `{fingers, directions: [{id, label}], actions: [{id, label,
// directions}], modifiers: [{id, label}]}`.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

QQC2.Dialog {
    id: editor

    property var choices: ({})
    /// `(index, gestureJson) => text`: the clash the gesture would make, "" for none. Null for no
    /// check.
    property var check: null
    /// Whether a request can be sent now (the page isn't busy).
    property bool canSave: true
    /// Whether a request to uQ is in flight. A save ends when it goes false.
    property bool busy: false
    /// `() => text`: the message of the last refused request, "" for none.
    property var errorNow: () => ""

    /// The gesture being changed; -1 for a new one.
    property int index: -1
    property int fingers: 3
    property string direction: "horizontal"
    property string action: "workspace"
    property string modifier: ""
    /// Whether the gesture is on; kept from the one being changed.
    property bool enabledNow: true
    property bool saving: false
    property string saveError: ""

    readonly property var actionChoices: editor.choices.actions || []
    readonly property var directionChoices: editor.choices.directions || []
    readonly property var modifierChoices: editor.choices.modifiers || []
    readonly property var fingerChoices: editor.choices.fingers || [3, 4, 5]
    readonly property var currentAction: editor.actionChoices.find(a => a.id === editor.action) || null
    /// The directions the action allows, in the table's order.
    readonly property var directions: {
        const ok = editor.currentAction ? editor.currentAction.directions : [];
        return editor.directionChoices.filter(d => ok.indexOf(d.id) >= 0);
    }
    /// Only the scratch workspace has a name.
    readonly property bool needsName: editor.action === "special"
    readonly property bool nameOk: !editor.needsName || nameBox.text === "" || /^[A-Za-z0-9_-]{1,32}$/.test(nameBox.text)
    /// The clash this gesture would make, "" when there is none.
    readonly property string warning: {
        if (typeof editor.check !== "function")
            return "";
        return String(editor.check(editor.index, JSON.stringify(editor.draft())) || "");
    }
    readonly property bool complete: editor.currentAction !== null && editor.directions.some(d => d.id === editor.direction) && editor.nameOk
    readonly property bool saveEnabled: editor.canSave && editor.complete && editor.warning === "" && !editor.saving
    readonly property alias nameField: nameBox
    readonly property alias fingersBox: fingersCombo
    readonly property alias directionBox: directionCombo
    readonly property alias actionBox: actionCombo
    readonly property alias modifierBox: modifierCombo
    readonly property alias warningMessage: clash
    readonly property alias saveButton: saveBtn

    /// Save was pressed: GestureList's `index` (-1 adds) and the gesture as JSON, for
    /// InputModel.saveGesture. The dialog stays open until the page calls finishSave().
    signal gestureAccepted(int index, string gestureJson)

    /// The gesture as uQ stores it.
    function draft() {
        const d = { fingers: editor.fingers, direction: editor.direction, action: editor.action, enabled: editor.enabledNow };
        if (editor.modifier !== "")
            d.modifier = editor.modifier;
        if (editor.needsName && nameBox.text !== "")
            d.workspace = nameBox.text;
        return d;
    }
    /// Fill the editor with gesture `at` of the list, or for -1 a new one.
    function load(at, gesture) {
        const g = gesture || {};
        editor.index = at;
        editor.fingers = g.fingers || 3;
        editor.action = g.action || "workspace";
        editor.direction = g.direction || "horizontal";
        editor.modifier = g.modifier ? String(g.modifier) : "";
        editor.enabledNow = g.enabled !== false;
        nameBox.text = g.workspace ? String(g.workspace) : "";
        editor.saving = false;
        editor.saveError = "";
        editor.keepDirection();
    }
    function startNew() {
        editor.load(-1, null);
        editor.open();
    }
    function change(at, gesture) {
        editor.load(at, gesture);
        editor.open();
    }
    /// After the action changed: a direction it doesn't allow becomes the first one it does.
    function keepDirection() {
        const ok = editor.directions;
        if (ok.length > 0 && !ok.some(d => d.id === editor.direction))
            editor.direction = ok[0].id;
    }
    function save() {
        if (!editor.saveEnabled)
            return;
        editor.saveError = "";
        editor.saving = true;
        editor.gestureAccepted(editor.index, JSON.stringify(editor.draft()));
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

    onActionChanged: editor.keepDirection()
    onChoicesChanged: editor.keepDirection()
    // A save ends when the request does, whether or not the page's state changed. Later, so the
    // request's error has been set by then.
    onBusyChanged: {
        if (!editor.busy && editor.saving)
            Qt.callLater(() => editor.finishSave(editor.errorNow()));
    }

    title: editor.index < 0 ? "Add a gesture" : "Change gesture"
    modal: true
    closePolicy: QQC2.Popup.CloseOnEscape
    anchors.centerIn: parent
    standardButtons: QQC2.Dialog.NoButton
    width: Math.min(Kirigami.Units.gridUnit * 28, parent ? parent.width : Kirigami.Units.gridUnit * 28)

    contentItem: ColumnLayout {
        id: body
        focus: true
        spacing: Kirigami.Units.smallSpacing
        Accessible.role: Accessible.Dialog
        Accessible.name: editor.title
        Keys.onReturnPressed: event => {
            editor.save();
            event.accepted = true;
        }
        Keys.onEnterPressed: event => {
            editor.save();
            event.accepted = true;
        }

        UqLabel {
            text: "Fingers"
        }
        UqComboBox {
            id: fingersCombo
            objectName: "fingersChoice"
            model: editor.fingerChoices.map(n => n + " fingers")
            currentIndex: Math.max(0, editor.fingerChoices.indexOf(editor.fingers))
            Accessible.name: "Fingers"
            Layout.fillWidth: true
            onActivated: i => editor.fingers = editor.fingerChoices[i]
        }
        UqLabel {
            text: "What you do"
        }
        UqComboBox {
            id: directionCombo
            objectName: "directionChoice"
            model: editor.directions.map(d => String(d.label))
            currentIndex: Math.max(0, editor.directions.findIndex(d => d.id === editor.direction))
            Accessible.name: "What you do"
            Layout.fillWidth: true
            onActivated: i => editor.direction = editor.directions[i].id
        }
        UqLabel {
            text: "What it does"
        }
        UqComboBox {
            id: actionCombo
            objectName: "actionChoice"
            model: editor.actionChoices.map(a => String(a.label))
            currentIndex: Math.max(0, editor.actionChoices.findIndex(a => a.id === editor.action))
            Accessible.name: "What it does"
            Layout.fillWidth: true
            onActivated: i => editor.action = editor.actionChoices[i].id
        }
        UqLabel {
            text: "Hold a key"
        }
        UqComboBox {
            id: modifierCombo
            objectName: "modifierChoice"
            model: editor.modifierChoices.map(m => String(m.label))
            currentIndex: Math.max(0, editor.modifierChoices.findIndex(m => m.id === editor.modifier))
            Accessible.name: "Hold a key"
            Layout.fillWidth: true
            onActivated: i => editor.modifier = editor.modifierChoices[i].id
        }
        UqLabel {
            objectName: "nameLabel"
            visible: editor.needsName
            text: "Name of the scratch workspace"
        }
        QQC2.TextField {
            id: nameBox
            objectName: "workspaceName"
            visible: editor.needsName
            maximumLength: 32
            placeholderText: "magic"
            Accessible.name: "Name of the scratch workspace"
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "nameHint"
            visible: editor.needsName && !editor.nameOk
            text: "Use letters, digits, - or _, up to 32."
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        RuleMessage {
            id: clash
            objectName: "conflictWarning"
            lines: editor.warning === "" ? [] : [editor.warning]
            negative: true
            closable: false
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "saveError"
            visible: editor.saveError !== ""
            text: editor.saveError
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
    }

    footer: QQC2.DialogButtonBox {
        QQC2.Button {
            id: saveBtn
            objectName: "saveButton"
            text: "Save"
            icon.name: "document-save"
            enabled: editor.saveEnabled
            onClicked: editor.save()
        }
        QQC2.Button {
            objectName: "cancelButton"
            text: "Cancel"
            onClicked: editor.close()
        }
    }
}
