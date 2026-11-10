// The settings a profile holds: each one's plain label and value, with Remove when the list is
// editable. Labels and values come from the profile (a file may have been imported), so they are
// shown through UqLabel only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts

ColumnLayout {
    id: list

    /// `[{domain, path, label, text}]`: the snapshot's `keys` rows of one profile.
    property var keys: []
    property bool editable: true

    /// Remove the setting `domain` / `pathJson` (the row's `path` as a JSON array).
    signal removeKey(string domain, string pathJson)

    spacing: 4

    UqLabel {
        objectName: "empty"
        text: "No settings in this profile yet"
        visible: list.keys.length === 0
        opacity: 0.7
        Layout.fillWidth: true
    }
    Repeater {
        model: list.keys

        RowLayout {
            id: row
            required property var modelData
            required property int index
            objectName: "keyRow" + index
            Layout.fillWidth: true

            UqLabel {
                objectName: "keyLabel"
                text: row.modelData.label
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            UqLabel {
                objectName: "keyText"
                text: row.modelData.text
                opacity: 0.7
                elide: Text.ElideRight
                Layout.maximumWidth: list.width / 2
            }
            QQC2.ToolButton {
                objectName: "remove"
                text: "Remove"
                icon.name: "list-remove"
                display: QQC2.AbstractButton.TextBesideIcon
                visible: list.editable
                Accessible.name: "Remove " + row.modelData.label
                onClicked: list.removeKey(row.modelData.domain, JSON.stringify(row.modelData.path))
            }
        }
    }
}
