// A button showing only its icon, named in a tooltip.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

QQC2.ToolButton {
    id: ib
    property string tip: ""
    display: QQC2.AbstractButton.IconOnly
    Accessible.name: ib.tip
    QQC2.ToolTip.text: ib.tip
    QQC2.ToolTip.visible: hovered
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
}
