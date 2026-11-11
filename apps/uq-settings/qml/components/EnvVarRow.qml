// One environment variable: its name and value, a warning for names that can stop apps from
// starting, where its value comes from, and whether a switch on Apps and scaling replaces it.
// Used on the Startup page (each row saved on its own) and in "Fix this app" (the dialog keeps
// the list). Names and values come from the person or from files, so the boxes hold them as
// plain text and every line about them is a UqLabel.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

ColumnLayout {
    id: row

    /// The saved name and value ("" for a new row).
    property string name
    property string value
    /// The name can stop apps from starting (the snapshot's `warn`).
    property bool warn: false
    /// uqd's `{source, detail}` for the variable, or null.
    property var origin: null
    /// The label of the switch that sets this name instead ("" for none).
    property string replacedBy
    /// Whether a change can be sent now (the page isn't busy).
    property bool idle: true
    /// **Save** is offered when the boxes differ from the saved name and value (the Startup
    /// page); without it, every change is reported through `edited` (the fix dialog).
    property bool showSave: true
    /// **Move up** and **Move down** are offered.
    property bool movable: false
    property bool canMoveUp: false
    property bool canMoveDown: false
    /// **Remove** is offered.
    property bool removable: true
    property int valueLimit: 4096
    /// Whether **Open user.lua** opens the file (tests turn it off).
    property bool opensFiles: true
    readonly property alias nameField: nameBox
    readonly property alias valueField: valueBox
    readonly property bool dirty: nameBox.text !== row.name || valueBox.text !== row.value

    /// The boxes changed (every keystroke).
    signal edited(string name, string value)
    /// **Save** was pressed.
    signal saveRequested(string name, string value)
    signal removeRequested()
    /// **Move up** (-1) or **Move down** (1).
    signal moveRequested(int by)

    /// Put the saved name and value back into the boxes (not a binding: typing would break it).
    function reset() {
        if (nameBox.text !== row.name)
            nameBox.text = row.name;
        if (valueBox.text !== row.value)
            valueBox.text = row.value;
    }

    spacing: 0
    Layout.fillWidth: true
    onNameChanged: row.reset()
    onValueChanged: row.reset()
    Component.onCompleted: row.reset()
    Accessible.role: Accessible.Grouping
    Accessible.name: row.name !== "" ? row.name : "New variable"

    RowLayout {
        spacing: Kirigami.Units.smallSpacing
        Layout.fillWidth: true

        QQC2.TextField {
            id: nameBox
            objectName: "varName"
            maximumLength: 128
            placeholderText: "NAME"
            Accessible.name: "Name"
            Accessible.description: "Capital letters, digits and _ only"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 10
            onTextEdited: row.edited(nameBox.text, valueBox.text)
        }
        UqLabel {
            text: "="
            Accessible.ignored: true
        }
        QQC2.TextField {
            id: valueBox
            objectName: "varValue"
            maximumLength: row.valueLimit
            placeholderText: "Value"
            Accessible.name: "Value"
            Accessible.description: nameBox.text
            Layout.fillWidth: true
            onTextEdited: row.edited(nameBox.text, valueBox.text)
            onAccepted: {
                if (row.showSave && row.dirty && row.idle)
                    row.saveRequested(nameBox.text.trim(), valueBox.text);
            }
        }
        QQC2.Button {
            objectName: "saveVar"
            visible: row.showSave && row.dirty
            enabled: row.idle && nameBox.text.trim() !== ""
            text: "Save"
            icon.name: "document-save"
            onClicked: row.saveRequested(nameBox.text.trim(), valueBox.text)
        }
        QQC2.ToolButton {
            visible: row.movable
            enabled: row.idle && row.canMoveUp
            text: "Move up"
            icon.name: "go-up"
            display: QQC2.AbstractButton.IconOnly
            Accessible.description: row.name
            onClicked: row.moveRequested(-1)
        }
        QQC2.ToolButton {
            visible: row.movable
            enabled: row.idle && row.canMoveDown
            text: "Move down"
            icon.name: "go-down"
            display: QQC2.AbstractButton.IconOnly
            Accessible.description: row.name
            onClicked: row.moveRequested(1)
        }
        QQC2.ToolButton {
            objectName: "removeVar"
            visible: row.removable
            enabled: row.idle
            text: "Remove"
            icon.name: "edit-delete"
            display: QQC2.AbstractButton.IconOnly
            Accessible.description: row.name
            onClicked: row.removeRequested()
        }
    }
    UqLabel {
        objectName: "varWarning"
        visible: row.warn
        text: "This can stop apps from starting. Remove it if apps break."
        color: Kirigami.Theme.neutralTextColor
        wrapMode: Text.Wrap
        Layout.fillWidth: true
    }
    UqLabel {
        objectName: "varReplaced"
        visible: row.replacedBy !== ""
        text: "Not used: the “" + row.replacedBy + "” switch on Apps and scaling sets this name."
        opacity: 0.7
        wrapMode: Text.Wrap
        Layout.fillWidth: true
    }
    SourceLine {
        source: row.origin
        textName: "varOrigin"
        opensFiles: row.opensFiles
        Layout.fillWidth: true
    }
}
