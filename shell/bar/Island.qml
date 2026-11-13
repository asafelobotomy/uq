import QtQuick
import "../core"
import "../logic/barlayout.js" as L

// A rounded floating panel; children go into the default content slot. Its width is its size:
// Icon (a square as tall as the bar), Small, Medium or Large (2, 4 and 6 times that). The bar
// sets `size` (stepping it down when the islands don't all fit); each island shows what fits
// that width (`room`, the inside width) and elides text rather than letting it run over.
Rectangle {
    id: root
    default property alias content: slot.data
    // False hides the island and its slot in the bar (Bar.qml), e.g. media with no player.
    property bool shown: true
    // `icon`, `small`, `medium` or `large`.
    property string size: "medium"
    // The width inside the border and padding, for the island's content.
    readonly property real room: width - Theme.gapIn * 2
    readonly property bool isIcon: size === "icon"
    // The island asks for edit mode with its own settings open (a "Change…" row in its popup).
    signal editRequested()

    color: Theme.surface
    radius: Theme.radius
    border.color: Theme.border
    border.width: Theme.borderWidth
    implicitHeight: BarLayout.unit
    implicitWidth: (L.UNITS[size] || 1) * BarLayout.unit
    // A last guard: content is laid out to fit, so this never cuts anything that was meant to
    // show; it only keeps a slow layout pass from drawing over a neighbour.
    clip: true

    Item {
        id: slot
        anchors.fill: parent
        anchors.margins: Theme.gapIn
    }
}
