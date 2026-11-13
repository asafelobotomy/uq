import QtQuick
import "../core"
import "../logic/barlayout.js" as L

// A rounded floating panel; children go into the default content slot. Its width is its size:
// Icon (a square as tall as the bar), Small, Medium or Large (2, 4 and 6 times that), or Custom
// (wider than Large, in whole Icon sizes). The bar sets `size` (stepping it down when the
// islands don't all fit) and the width; a Custom island gets `size` "large" (it shows what it
// shows at Large, with more room). Each island shows what fits its width (`room`, the inside
// width) and elides text rather than letting it run over.
Rectangle {
    id: root
    default property alias content: slot.data
    // False hides the island and its slot in the bar (Bar.qml), e.g. media with no player.
    property bool shown: true
    // `icon`, `small`, `medium` or `large` (Custom islands get `large`).
    property string size: "medium"
    // The width inside the border and padding, for the island's content.
    readonly property real room: width - Theme.gapIn * 2
    readonly property bool isIcon: size === "icon"
    // The island asks for edit mode with its own settings open (a "Change…" row in its popup).
    signal editRequested()
    // The island asks for the uQ Island Menu (an island whose own right click does something
    // else offers it there), under `anchor`.
    signal menuRequested(Item anchor)

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
