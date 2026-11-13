// A small "?" beside a setting: its longer explanation in a tooltip, on hover or keyboard focus,
// so the page itself stays short.
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

QQC2.ToolButton {
    id: tip

    /// The explanation: one or two short sentences.
    property string tip: ""

    visible: tip.tip !== ""
    icon.name: "help-contextual"
    display: QQC2.AbstractButton.IconOnly
    flat: true
    implicitWidth: Kirigami.Units.iconSizes.small + Kirigami.Units.smallSpacing * 2
    implicitHeight: implicitWidth
    focusPolicy: Qt.TabFocus
    Accessible.name: "More about this"
    Accessible.description: tip.tip

    QQC2.ToolTip.visible: tip.hovered || tip.activeFocus || tip.pressed
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
    QQC2.ToolTip.text: tip.tip
}
