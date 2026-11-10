// The keyboard layouts in use, in order (the first is the default), with Move up, Move down and
// Remove on each, and an Add row. Layout and variant names are plain text (UqLabel). Without
// the system's layout list, the Add row takes a layout code instead.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts

ColumnLayout {
    id: list

    /// `[{layout, variant?}]`, as stored.
    property var layouts: []
    /// `[[code, name]]` from the system's list; empty when it couldn't be read.
    property var xkbLayouts: []
    /// `{layoutCode: [[code, name]]}`.
    property var variants: ({})
    readonly property var chosenVariants: addLayout.currentIndex > 0 ? (variants[xkbLayouts[addLayout.currentIndex - 1][0]] || []) : []
    readonly property alias addButton: addBtn
    readonly property alias layoutPicker: addLayout
    readonly property alias codeField: codeField

    /// The whole new list, after a move.
    signal reordered(var layouts)
    signal removed(int index)
    signal added(string layout, string variant)

    function nameOf(entry) {
        const l = xkbLayouts.find(x => x[0] === entry.layout);
        let name = l ? l[1] : entry.layout;
        if (entry.variant) {
            const v = (variants[entry.layout] || []).find(x => x[0] === entry.variant);
            name += " – " + (v ? v[1] : entry.variant);
        }
        return name;
    }
    function moved(from, to) {
        const next = layouts.slice();
        const [item] = next.splice(from, 1);
        next.splice(to, 0, item);
        return next;
    }

    spacing: 4

    Repeater {
        model: list.layouts

        RowLayout {
            id: row
            required property var modelData
            required property int index
            objectName: "layoutRow" + index
            Layout.fillWidth: true

            UqLabel {
                text: list.nameOf(row.modelData) + (row.index === 0 ? " (default)" : "")
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            QQC2.ToolButton {
                objectName: "up"
                text: "Up"
                icon.name: "go-up"
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: row.index > 0
                Accessible.name: "Move up"
                onClicked: list.reordered(list.moved(row.index, row.index - 1))
            }
            QQC2.ToolButton {
                objectName: "down"
                text: "Down"
                icon.name: "go-down"
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: row.index < list.layouts.length - 1
                Accessible.name: "Move down"
                onClicked: list.reordered(list.moved(row.index, row.index + 1))
            }
            QQC2.ToolButton {
                objectName: "remove"
                text: "Remove"
                icon.name: "list-remove"
                display: QQC2.AbstractButton.TextBesideIcon
                Accessible.name: "Remove"
                onClicked: list.removed(row.index)
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        UqComboBox {
            id: addLayout
            visible: list.xkbLayouts.length > 0
            model: ["Choose a layout"].concat(list.xkbLayouts.map(l => l[1]))
            currentIndex: 0
            Accessible.name: "Layout to add"
            Layout.fillWidth: true
            onActivated: addVariant.currentIndex = 0
        }
        UqComboBox {
            id: addVariant
            visible: list.chosenVariants.length > 0
            model: ["Standard"].concat(list.chosenVariants.map(v => v[1]))
            currentIndex: 0
            Accessible.name: "Variant"
        }
        QQC2.TextField {
            id: codeField
            visible: list.xkbLayouts.length === 0
            placeholderText: "Layout code, like us or de"
            Accessible.name: "Layout code"
            Layout.fillWidth: true
        }
        QQC2.Button {
            id: addBtn
            text: "Add"
            enabled: list.layouts.length < 4 && (list.xkbLayouts.length > 0 ? addLayout.currentIndex > 0 : codeField.text.trim() !== "")
            onClicked: {
                if (list.xkbLayouts.length > 0) {
                    const code = list.xkbLayouts[addLayout.currentIndex - 1][0];
                    const v = addVariant.currentIndex > 0 ? list.chosenVariants[addVariant.currentIndex - 1][0] : "";
                    list.added(code, v);
                    addLayout.currentIndex = 0;
                } else {
                    list.added(codeField.text.trim(), "");
                    codeField.text = "";
                }
            }
        }
    }
}
