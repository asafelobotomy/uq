import QtQuick
import "../logic/keys.js" as KeyRules

// The only TextInput in the shell: plain single-line text, no predictive text, length-capped.
// `secret` makes it a password field (echo off, sensitive-data hints, room for long passwords,
// and shortcuts such as SUPER+N are not typed into it: logic/keys.js).
// scripts/lint-shell.sh forbids a bare TextInput outside this file.
Item {
    id: root
    property alias text: input.text
    property string placeholder: ""
    property bool secret: false
    property int maximumLength: secret ? 512 : 100
    signal edited(string text)
    signal accepted()
    implicitHeight: 26
    implicitWidth: 180

    Rectangle {
        anchors.fill: parent
        radius: Theme.radius - 2
        color: Theme.surfaceAlt
        border.color: input.activeFocus ? Theme.text : Theme.border
        border.width: Theme.borderWidth
    }
    TextInput {
        id: input
        anchors.fill: parent
        anchors.leftMargin: Theme.gapIn
        anchors.rightMargin: Theme.gapIn
        verticalAlignment: TextInput.AlignVCenter
        clip: true
        maximumLength: root.maximumLength
        echoMode: root.secret ? TextInput.Password : TextInput.Normal
        inputMethodHints: root.secret ? (Qt.ImhSensitiveData | Qt.ImhNoPredictiveText | Qt.ImhHiddenText)
                                      : Qt.ImhNoPredictiveText
        color: Theme.text
        font.family: Theme.fontUi
        font.pixelSize: Theme.fontSize
        selectByMouse: !root.secret
        onTextEdited: root.edited(text)
        onAccepted: root.accepted()
        Keys.onPressed: event => {
            if (root.secret && KeyRules.blocks(event.modifiers, event.key))
                event.accepted = true;
        }
        // Screen readers see the focused TextInput: it carries the name set on this item.
        Accessible.role: Accessible.EditableText
        Accessible.name: root.Accessible.name
        Accessible.passwordEdit: root.secret
    }
    UqText {
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        anchors.leftMargin: Theme.gapIn
        visible: input.text === "" && root.placeholder !== ""
        text: root.placeholder
        color: Theme.textMuted
    }
    function forceActiveFocus() { input.forceActiveFocus(); }
    function clear() { input.clear(); }
}
