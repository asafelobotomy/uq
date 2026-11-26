// A layout drawn as its windows, small.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Item {
    id: drawing
    property string layout: "dwindle"
    /// The windows as `[x, y, w, h]` fractions.
    property var shapes: []
    property bool active: false
    clip: true
    implicitWidth: Kirigami.Units.gridUnit * 5
    implicitHeight: Kirigami.Units.gridUnit * 3
    Accessible.ignored: true
    Repeater {
        model: drawing.shapes
        Rectangle {
            required property var modelData
            required property int index
            x: modelData[0] * drawing.width + 1
            y: modelData[1] * drawing.height + 1
            width: modelData[2] * drawing.width - 2
            height: modelData[3] * drawing.height - 2
            radius: Kirigami.Units.cornerRadius / 2
            color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.12)
            border.width: 1
            border.color: drawing.active && index === (drawing.layout === "scrolling" ? 1 : 0) ? Kirigami.Theme.highlightColor : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.3)
        }
    }
}
