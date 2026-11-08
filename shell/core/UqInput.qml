import QtQuick

// The only TextInput in the shell: plain single-line text, no predictive text, length-capped.
// scripts/lint-shell.sh forbids a bare TextInput outside this file.
Item {
    id: root
    property alias text: input.text
    property string placeholder: ""
    property alias maximumLength: input.maximumLength
    signal edited(string text)
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
        maximumLength: 100
        inputMethodHints: Qt.ImhNoPredictiveText
        color: Theme.text
        font.family: Theme.fontUi
        font.pixelSize: Theme.fontSize
        selectByMouse: true
        onTextEdited: root.edited(text)
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
}
