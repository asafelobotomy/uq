// A list the user can reorder, with a switch per row (optional) and a remove button (optional).
// `model` is an array of { id, name, enabled, missing }; names are shown through UqLabel only.
// The list never changes itself: it asks (reordered, toggled, removed) and shows `model` again
// once the page has the new state.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

ColumnLayout {
    id: root

    property var model: []
    property bool showSwitch: true
    property bool removable: false
    /// The ids in their new order.
    signal reordered(var ids)
    signal toggled(string id, bool on)
    signal removed(string id)
    readonly property alias repeater: rep

    function ids() {
        return model.map(m => m.id);
    }
    function move(from, to) {
        const list = ids();
        list.splice(to, 0, list.splice(from, 1)[0]);
        reordered(list);
    }

    spacing: Kirigami.Units.smallSpacing

    Repeater {
        id: rep
        model: root.model

        RowLayout {
            id: row

            required property int index
            required property var modelData
            readonly property alias upButton: up
            readonly property alias downButton: down
            readonly property alias removeButton: remove
            readonly property alias toggle: sw

            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true

            QQC2.Switch {
                id: sw
                visible: root.showSwitch
                checked: row.modelData.enabled !== false
                enabled: root.enabled
                Accessible.name: row.modelData.name
                onToggled: {
                    root.toggled(row.modelData.id, sw.checked);
                    sw.checked = Qt.binding(() => row.modelData.enabled !== false);
                }
            }
            UqLabel {
                text: row.modelData.name
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Not installed"
                visible: row.modelData.missing === true
                opacity: 0.7
            }
            QQC2.ToolButton {
                id: up
                icon.name: "go-up"
                enabled: root.enabled && row.index > 0
                Accessible.name: "Move " + row.modelData.name + " up"
                onClicked: root.move(row.index, row.index - 1)
            }
            QQC2.ToolButton {
                id: down
                icon.name: "go-down"
                enabled: root.enabled && row.index < root.model.length - 1
                Accessible.name: "Move " + row.modelData.name + " down"
                onClicked: root.move(row.index, row.index + 1)
            }
            QQC2.ToolButton {
                id: remove
                visible: root.removable
                icon.name: "list-remove"
                enabled: root.enabled
                Accessible.name: "Remove " + row.modelData.name
                onClicked: root.removed(row.modelData.id)
            }
        }
    }
}
