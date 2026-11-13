// A button that shows a value or a choice ("60 Hz", "14:30", "Light"), optionally with an icon,
// with even padding and a minimum width so a row of them reads as a set rather than as cramped
// labels. Usually checkable, in a ButtonGroup.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

QQC2.Button {
    id: button

    /// What the button says (any text: it is drawn through UqLabel).
    property string label: ""
    property string iconName: ""
    /// Also shown as a tooltip, for a button whose label is a value ("24-hour" for "14:30").
    property string tip: ""

    leftPadding: Kirigami.Units.largeSpacing * 1.5
    rightPadding: Kirigami.Units.largeSpacing * 1.5
    topPadding: Kirigami.Units.smallSpacing * 1.5
    bottomPadding: Kirigami.Units.smallSpacing * 1.5
    implicitWidth: Math.max(Kirigami.Units.gridUnit * 4, implicitContentWidth + leftPadding + rightPadding)
    Accessible.name: button.tip !== "" ? button.tip : button.label

    QQC2.ToolTip.visible: button.tip !== "" && button.hovered
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
    QQC2.ToolTip.text: button.tip

    contentItem: RowLayout {
        spacing: Kirigami.Units.smallSpacing
        Item {
            Layout.fillWidth: true
        }
        Kirigami.Icon {
            visible: button.iconName !== ""
            source: button.iconName
            implicitWidth: Kirigami.Units.iconSizes.small
            implicitHeight: Kirigami.Units.iconSizes.small
            Accessible.ignored: true
        }
        UqLabel {
            visible: button.label !== ""
            text: button.label
            font.bold: button.checked
            horizontalAlignment: Text.AlignHCenter
        }
        Item {
            Layout.fillWidth: true
        }
    }
}
