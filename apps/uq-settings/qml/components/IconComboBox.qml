// A drop-down whose choices each have an icon before their name: what each choice does, at a
// glance (Power's lid actions, Default apps' apps). `model` is a list of `{text, icon}`. The shown
// value and the rows are UqLabels (plain text), as UqComboBox; names may come from other
// packages' files.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

QQC2.ComboBox {
    id: combo

    /// The icon for a choice with none of its own.
    property string fallbackIcon: ""
    readonly property var current: combo.currentIndex >= 0 && combo.currentIndex < combo.count ? combo.model[combo.currentIndex] : null

    function iconOf(item) {
        const i = item && item.icon ? String(item.icon) : "";
        return i !== "" ? i : combo.fallbackIcon;
    }

    textRole: "text"
    displayText: ""
    Layout.maximumWidth: Kirigami.Units.gridUnit * 20
    Accessible.role: Accessible.ComboBox
    Accessible.description: combo.current ? String(combo.current.text) : ""

    contentItem: RowLayout {
        // As UqComboBox: the style asks the contentItem for a text cursor rectangle.
        readonly property int selectionStart: 0
        function positionToRectangle(position: int): rect {
            return Qt.rect(0, 0, 0, 0);
        }
        spacing: Kirigami.Units.smallSpacing

        Kirigami.Icon {
            visible: combo.current !== null && combo.iconOf(combo.current) !== ""
            source: combo.iconOf(combo.current)
            implicitWidth: Kirigami.Units.iconSizes.small
            implicitHeight: Kirigami.Units.iconSizes.small
            Layout.leftMargin: Kirigami.Units.smallSpacing
            Accessible.ignored: true
        }
        UqLabel {
            text: combo.current ? String(combo.current.text) : ""
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
            Layout.fillWidth: true
            Layout.leftMargin: combo.current && combo.iconOf(combo.current) !== "" ? 0 : Kirigami.Units.smallSpacing
            Layout.rightMargin: combo.indicator ? combo.indicator.width + Kirigami.Units.smallSpacing : 0
        }
    }
    popup.width: Math.max(combo.width, Math.min(popup.implicitWidth, Kirigami.Units.gridUnit * 30))
    delegate: QQC2.MenuItem {
        id: row
        required property int index
        required property var modelData
        width: ListView.view ? ListView.view.width : implicitWidth
        highlighted: combo.highlightedIndex === index
        Accessible.name: String(row.modelData.text)
        contentItem: RowLayout {
            spacing: Kirigami.Units.smallSpacing
            Kirigami.Icon {
                visible: combo.iconOf(row.modelData) !== ""
                source: combo.iconOf(row.modelData)
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Accessible.ignored: true
            }
            UqLabel {
                text: String(row.modelData.text)
                elide: Text.ElideRight
                verticalAlignment: Text.AlignVCenter
                highlighted: row.highlighted || row.down
                Layout.fillWidth: true
                Layout.leftMargin: combo.iconOf(row.modelData) !== "" ? 0 : Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
            }
        }
    }
}
