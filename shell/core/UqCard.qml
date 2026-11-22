import QtQuick

// A shell surface's card: the theme's surface, border and corners, and one way in and out for
// every popup. While `shown` turns true it fades in and grows from Motion.popScale on the side
// it opens from (`origin`, an Item.TransformOrigin; Top for popups under the bar); turning false
// it fades out faster. All of it is instant under ui.reduce_motion. The card hides itself once
// faded out, so a window that stays mapped (app menu, quick settings, power) draws nothing.
Rectangle {
    id: card

    property bool shown: true
    property int origin: Item.Top
    // 0 hidden .. 1 shown; animated, so the card is part-way in while it opens.
    property real reveal: shown ? 1 : 0

    Behavior on reveal {
        NumberAnimation {
            duration: card.shown ? Motion.base : Motion.fast
            easing.type: card.shown ? Motion.easing : Motion.easingExit
        }
    }

    opacity: reveal
    visible: shown || reveal > 0
    scale: Motion.popScale + (1 - Motion.popScale) * reveal
    transformOrigin: origin
    radius: Theme.radius
    color: Theme.surface
    border.color: Theme.border
    border.width: Theme.borderWidth
}
