// The keyboard layouts in use, in order (the first is the default), drawn as keycaps: the
// layout's code large on the cap, its name under it, and Move earlier, Move later and Remove on
// each. An Add row follows. Layout and variant names are plain text (UqLabel). Without the
// system's layout list, the Add row takes a layout code instead.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

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

    spacing: Kirigami.Units.largeSpacing

    UqLabel {
        objectName: "noLayouts"
        visible: list.layouts.length === 0
        text: "The standard US layout"
        opacity: 0.7
    }

    Flow {
        visible: list.layouts.length > 0
        spacing: Kirigami.Units.largeSpacing
        Layout.fillWidth: true

        Repeater {
            model: list.layouts

            ColumnLayout {
                id: row
                required property var modelData
                required property int index
                readonly property string name: list.nameOf(row.modelData)
                objectName: "layoutRow" + index
                width: Kirigami.Units.gridUnit * 9
                spacing: Kirigami.Units.smallSpacing
                Accessible.role: Accessible.Grouping
                Accessible.name: row.name + (row.index === 0 ? ", the default" : "")

                // The keycap: the code large, the default one in the highlight colour.
                Rectangle {
                    id: cap
                    Layout.alignment: Qt.AlignHCenter
                    implicitWidth: Kirigami.Units.gridUnit * 4
                    implicitHeight: Kirigami.Units.gridUnit * 3
                    radius: Kirigami.Units.cornerRadius * 2
                    color: row.index === 0 ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.2) : Kirigami.Theme.backgroundColor
                    border.width: row.index === 0 ? 2 : 1
                    border.color: row.index === 0 ? Kirigami.Theme.highlightColor : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.25)

                    // The key's lower edge, so it reads as a key.
                    Rectangle {
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.bottom: parent.bottom
                        anchors.margins: 1
                        height: Kirigami.Units.smallSpacing
                        radius: Kirigami.Units.cornerRadius
                        color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.08)
                    }
                    UqLabel {
                        anchors.centerIn: parent
                        text: String(row.modelData.layout).toUpperCase()
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.6
                        font.bold: true
                    }
                    Kirigami.Icon {
                        visible: row.index === 0
                        source: "favorite"
                        anchors.top: parent.top
                        anchors.right: parent.right
                        anchors.margins: Kirigami.Units.smallSpacing
                        implicitWidth: Kirigami.Units.iconSizes.small
                        implicitHeight: Kirigami.Units.iconSizes.small
                        Accessible.ignored: true
                    }
                }
                UqLabel {
                    objectName: "layoutName"
                    text: row.name
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                    opacity: 0.8
                    Layout.fillWidth: true
                }
                RowLayout {
                    spacing: 0
                    Layout.alignment: Qt.AlignHCenter

                    QQC2.ToolButton {
                        objectName: "up"
                        text: "Move earlier"
                        icon.name: "go-previous"
                        display: QQC2.AbstractButton.IconOnly
                        enabled: row.index > 0
                        Accessible.name: "Move " + row.name + " earlier"
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.text: "Move earlier"
                        onClicked: list.reordered(list.moved(row.index, row.index - 1))
                    }
                    QQC2.ToolButton {
                        objectName: "down"
                        text: "Move later"
                        icon.name: "go-next"
                        display: QQC2.AbstractButton.IconOnly
                        enabled: row.index < list.layouts.length - 1
                        Accessible.name: "Move " + row.name + " later"
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.text: "Move later"
                        onClicked: list.reordered(list.moved(row.index, row.index + 1))
                    }
                    QQC2.ToolButton {
                        objectName: "remove"
                        text: "Remove"
                        icon.name: "list-remove"
                        display: QQC2.AbstractButton.IconOnly
                        Accessible.name: "Remove " + row.name
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.text: "Remove"
                        onClicked: list.removed(row.index)
                    }
                }
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
            Layout.fillWidth: true
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
            icon.name: "list-add"
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
