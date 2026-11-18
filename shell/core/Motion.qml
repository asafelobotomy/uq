pragma Singleton
import QtQuick

// The shell's motion tokens: one speed and one easing for UI transitions, all zero under
// ui.reduce_motion. Use these instead of literal millisecond counts so the shell moves alike.
QtObject {
    // Hover and focus feedback.
    readonly property int fast: Settings.reduceMotion ? 0 : 100
    // Moves, resizes and slides.
    readonly property int base: Settings.reduceMotion ? 0 : 140
    readonly property int easing: Easing.OutCubic
}
