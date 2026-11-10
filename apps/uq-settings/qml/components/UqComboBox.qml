// A ComboBox whose shown value and popup rows are UqLabels (plain text). The style would paint
// model strings itself, possibly as rich text; scripts/lint-control-text.awk requires every
// stock ComboBox to declare both contentItem and delegate, so use this instead of QQC2.ComboBox.
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import "."

QQC2.ComboBox {
    id: combo

    // The desktop style paints `displayText` itself (rich-text capable); blank it so only our
    // UqLabel below shows the value, taken from currentText.
    displayText: ""
    contentItem: UqLabel {
        text: combo.currentText
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
        leftPadding: Kirigami.Units.smallSpacing
        rightPadding: combo.indicator ? combo.indicator.width + Kirigami.Units.smallSpacing : 0
    }
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
}
