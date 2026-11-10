// A setting that is either "Use theme" (null) or one name from a list, such as a font family.
// `value` is the current override (null or "" = use the theme). The combo shows `value` again
// straight after the user picks, until the model confirms the change, so a refused change
// leaves it showing the last good value.
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import "."

SettingRow {
    id: root

    property var choices: []
    property var value: null
    property string themeLabel: "Use theme"
    property bool editable: true
    /// The new value: a string, or null for "Use theme".
    signal chosen(var value)

    readonly property var items: {
        const list = [themeLabel].concat(choices);
        if (value && choices.indexOf(value) < 0)
            list.push(value);
        return list;
    }

    function shownIndex() {
        return value ? items.indexOf(value) : 0;
    }

    QQC2.ComboBox {
        id: combo
        model: root.items
        currentIndex: root.shownIndex()
        enabled: root.enabled
        implicitContentWidthPolicy: QQC2.ComboBox.WidestText
        Accessible.name: root.label
        // The style paints the shown name itself, as plain text; the popup rows are ours.
        delegate: QQC2.ItemDelegate {
            required property int index
            required property var modelData
            width: combo.width
            highlighted: combo.highlightedIndex === index
            contentItem: UqLabel {
                text: modelData
                elide: Text.ElideRight
            }
        }
        onActivated: index => {
            root.chosen(index === 0 ? null : root.items[index]);
            combo.currentIndex = Qt.binding(() => root.shownIndex());
        }
    }
}
