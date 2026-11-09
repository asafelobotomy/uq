import QtQuick

// Feeds one IdleMonitor. Quickshell 0.3.2's IdleMonitor reads `timeout` only when it becomes
// enabled; a new timeout on an enabled monitor is ignored. So when the timeout changes, `armed`
// drops at once and comes back on the next turn, and the monitor re-reads the new timeout. That
// covers a setting changed at runtime and saved timings that arrive from uqd after startup.
QtObject {
    id: r

    // Whether the stage should run (idle.js enabled()).
    property bool want: false
    // Seconds.
    property int timeout: 0
    property bool holding: false
    // Bind the monitor's `enabled` to this and its `timeout` to `timeout`.
    readonly property bool armed: want && !holding

    onTimeoutChanged: {
        holding = true;
        turn.restart();
    }

    property Timer turn: Timer {
        id: turn
        interval: 0
        onTriggered: r.holding = false
    }
}
