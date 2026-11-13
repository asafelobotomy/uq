import QtQuick
import "../../core"

// A thin rule between islands (`separator`); any number may be placed. Not an island itself:
// no panel, just the theme's border colour at its border width.
Item {
    id: root
    property string size: "normal"
    property bool shown: true
    implicitWidth: Theme.borderWidth + Theme.gapIn * 2
    implicitHeight: 36
    Accessible.role: Accessible.Separator
    Accessible.name: "Separator"

    Rectangle {
        anchors.centerIn: parent
        width: Theme.borderWidth
        height: parent.height * (root.size === "compact" ? 0.4 : (root.size === "wide" ? 0.9 : 0.6))
        radius: width / 2
        color: Theme.border
    }
}
