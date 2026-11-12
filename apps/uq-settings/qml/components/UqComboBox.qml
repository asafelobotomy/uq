// A ComboBox whose shown value and popup rows are UqLabels (plain text). The style would paint
// model strings itself, possibly as rich text; scripts/lint-control-text.awk requires every
// stock ComboBox to declare both contentItem and delegate, so use this instead of QQC2.ComboBox.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

QQC2.ComboBox {
    id: combo

    // The desktop style paints `displayText` itself (rich-text capable); blank it so only our
    // UqLabel below shows the value, taken from currentText.
    displayText: ""
    // displayText is blanked, so the accessible value comes from here; callers set Accessible.name.
    // Wide enough for nearly every choice; a longer one is cut short with "…". Without a cap, a
    // box sized to its widest choice (WidestText) starved its row's label: on Input, "Switch
    // layouts with" wrapped to two letters a line. A caller can still set its own.
    Layout.maximumWidth: Kirigami.Units.gridUnit * 20
    Accessible.role: Accessible.ComboBox
    Accessible.description: currentText
    contentItem: UqLabel {
        // qqc2-desktop-style's ComboBox places a touch selection handle on its contentItem by
        // calling contentItem.positionToRectangle(selectionStart), expecting the TextField of an
        // editable box. A label has no cursor: answer with an empty rectangle, so that binding
        // doesn't throw a TypeError for every box on every page.
        readonly property int selectionStart: 0
        function positionToRectangle(position: int): rect {
            return Qt.rect(0, 0, 0, 0);
        }
        // Binds currentText because displayText is blanked above.
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
