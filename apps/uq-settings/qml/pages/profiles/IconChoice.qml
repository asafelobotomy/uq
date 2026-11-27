// A row of the offered icons; `chosen` is the picked icon name. An inline component can't see
// the page's ids, so it takes the icons and their names as properties.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Flow {
    id: choice

    property var icons: []
    property var names: ({})
    property string chosen

    spacing: Kirigami.Units.smallSpacing

    Repeater {
        model: choice.icons
        delegate: QQC2.ToolButton {
            required property string modelData

            icon.name: modelData
            display: QQC2.AbstractButton.IconOnly
            checkable: true
            checked: choice.chosen === modelData
            Accessible.name: choice.names[modelData] || modelData
            QQC2.ToolTip.text: choice.names[modelData] || modelData
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            onClicked: choice.chosen = modelData
        }
    }
}
