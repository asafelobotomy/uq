// One layout's options: a heading row and the option rows under it. It is shown when a workspace
// or the default uses the layout (`inUse`), or when a search hit points at it (`highlight` is its
// `settingId`), and then says nothing uses the layout yet.
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

ColumnLayout {
    id: root

    property string settingId
    property string label
    property string highlight
    property bool inUse: false
    default property alias content: slot.data
    readonly property bool shown: inUse || (highlight !== "" && highlight === settingId)

    visible: shown
    spacing: Kirigami.Units.smallSpacing

    SettingRow {
        settingId: root.settingId
        label: root.label
        highlight: root.highlight
        enabled: root.enabled
        Layout.fillWidth: true
        UqLabel {
            objectName: "unusedNote"
            text: "Not used by any workspace yet"
            visible: !root.inUse
        }
    }
    ColumnLayout {
        id: slot
        spacing: Kirigami.Units.smallSpacing
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing * 2
    }
}
