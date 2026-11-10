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
    /// Shown names for some choices, by choice: `{ master: "Master" }`; others show as they are.
    property var labels: ({})
    /// The new value: a string, or null for "Use theme".
    signal chosen(var value)

    readonly property var items: {
        const list = [themeLabel].concat(choices);
        if (value && choices.indexOf(value) < 0)
            list.push(value);
        return list;
    }

    function labelOf(item) {
        return labels[item] !== undefined ? labels[item] : item;
    }

    function shownIndex() {
        return value ? items.indexOf(value) : 0;
    }

    UqComboBox {
        id: combo
        model: root.items.map(root.labelOf)
        currentIndex: root.shownIndex()
        enabled: root.enabled
        implicitContentWidthPolicy: QQC2.ComboBox.WidestText
        Accessible.name: root.label
        onActivated: index => {
            root.chosen(index === 0 ? null : root.items[index]);
            combo.currentIndex = Qt.binding(() => root.shownIndex());
        }
    }
}
