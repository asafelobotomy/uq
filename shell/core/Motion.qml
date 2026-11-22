pragma Singleton
import QtQuick

// The shell's motion tokens, all zero under ui.reduce_motion. Use these instead of literal
// millisecond counts so the shell moves alike; UqCard uses them for every popup's way in and out.
QtObject {
    // Hover, press and focus feedback, and a popup's way out.
    readonly property int fast: Settings.reduceMotion ? 0 : 100
    // Moves, resizes, slides and a popup's way in.
    readonly property int base: Settings.reduceMotion ? 0 : 140
    // Large surfaces (a whole-screen scrim, a panel growing by a page).
    readonly property int slow: Settings.reduceMotion ? 0 : 220
    // Things arriving or moving: they start quick and settle.
    readonly property int easing: Easing.OutCubic
    // Things leaving: they start slow and get out of the way.
    readonly property int easingExit: Easing.InCubic
    // How small a popup starts before it grows to full size, from the side it opens from.
    readonly property real popScale: Settings.reduceMotion ? 1 : 0.96
}
