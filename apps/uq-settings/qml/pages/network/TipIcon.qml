// A small icon that names itself in a tooltip on hover.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Kirigami.Icon {
    id: tipIcon
    property string tip: ""
    implicitWidth: Kirigami.Units.iconSizes.small
    implicitHeight: Kirigami.Units.iconSizes.small
    Accessible.name: tipIcon.tip
    HoverHandler {
        id: tipHover
    }
    QQC2.ToolTip.visible: tipHover.hovered && tipIcon.tip !== ""
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
    QQC2.ToolTip.text: tipIcon.tip
}
