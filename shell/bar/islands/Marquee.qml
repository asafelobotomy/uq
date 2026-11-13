import QtQuick
import "../../core"
import "../../logic/ticker.js" as T

// Scrolling text for the ticker islands: `model` (an array) drawn by `delegate`, side by side.
// It moves only when the items don't fit, and stops while the pointer is over it, while it has
// keyboard focus, while the screen is shared, and when the user asked for reduced motion. With
// reduced motion (or while sharing) it shows one item at a time and steps to the next every
// `stepMs`, with no movement. Pure QML: loads under qmltestrunner.
Item {
    id: root

    property var model: []
    property Component delegate: null
    // Pixels per second.
    property real speed: Theme.fontSize * 4
    property int stepMs: 8000
    property bool reduceMotion: Settings.reduceMotion
    property bool sharing: Bridge.sharing
    property bool focused: activeFocus
    readonly property bool hovered: hover.hovered

    readonly property int count: model ? model.length : 0
    readonly property real gap: Theme.gapOut * 2
    readonly property real stripWidth: one.width
    readonly property bool overflows: count > 0 && stripWidth > width
    readonly property bool stepping: overflows && (reduceMotion || sharing)
    readonly property bool moving: T.moving(overflows, reduceMotion, hovered, focused, sharing)
    property int stepIndex: 0

    implicitHeight: one.height
    clip: true

    onModelChanged: stepIndex = 0

    HoverHandler {
        id: hover
    }

    Timer {
        interval: root.stepMs
        repeat: true
        running: root.stepping && !root.hovered && root.count > 1
        onTriggered: root.stepIndex = (root.stepIndex + 1) % root.count
    }

    // One step's item, when stepping.
    Row {
        id: single
        visible: root.stepping
        anchors.verticalCenter: parent.verticalCenter
        Repeater {
            model: root.stepping && root.count > 0 ? [root.model[Math.min(root.stepIndex, root.count - 1)]] : []
            delegate: root.delegate
        }
    }

    // The strip: the items, then (while it overflows) the same again, for a seamless loop.
    Row {
        id: strip
        visible: !root.stepping
        anchors.verticalCenter: parent.verticalCenter
        spacing: root.gap
        Row {
            id: one
            spacing: root.gap
            Repeater {
                model: root.model
                delegate: root.delegate
            }
        }
        Row {
            visible: root.overflows
            spacing: root.gap
            Repeater {
                model: root.overflows ? root.model : []
                delegate: root.delegate
            }
        }
    }

    NumberAnimation {
        id: scroll
        target: strip
        property: "x"
        from: 0
        to: -(root.stripWidth + root.gap)
        duration: T.scrollMs(root.stripWidth + root.gap, root.speed)
        loops: Animation.Infinite
        running: root.overflows && !root.stepping
        paused: running && !root.moving
    }

    onOverflowsChanged: if (!overflows) strip.x = 0
    onSteppingChanged: strip.x = 0
}
