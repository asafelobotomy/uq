// Keys to know: each row is the keys, then what they do. `binds` is `[{keys, does}]` from
// WelcomeModel (kept true to config/hypr/base.lua by a test). Plain text only.
import QtQuick
import QtQuick.Layouts

GridLayout {
    id: list

    property var binds: []

    columns: 2
    columnSpacing: 16
    rowSpacing: 6
    Accessible.role: Accessible.Table
    Accessible.name: "Keys to know"

    Repeater {
        model: list.binds
        delegate: UqLabel {
            required property var modelData
            required property int index
            // Two cells per bind: the keys, then what they do.
            text: modelData.keys
            font.bold: true
            Layout.row: index
            Layout.column: 0
        }
    }
    Repeater {
        model: list.binds
        delegate: UqLabel {
            required property var modelData
            required property int index
            text: modelData.does
            wrapMode: Text.Wrap
            Layout.row: index
            Layout.column: 1
            Layout.fillWidth: true
        }
    }
}
