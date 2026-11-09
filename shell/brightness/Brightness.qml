pragma Singleton
import QtQuick
import Quickshell.Io
import "../logic/quick.js" as Q
import "../logic/quickui.js" as U

// Backlight via `brightnessctl` with a fixed argv (spec §2.2); never a shell. One Process at a
// time: a step while one is running, or with no backlight device, is a no-op, so key-repeat
// never queues processes. A missing brightnessctl or no backlight just leaves `available` false,
// with one log line at most and no retry.
QtObject {
    id: root

    // --- public state ---
    property bool available: false
    property int percent: 0

    // Emitted after a successful step, with the new percentage.
    signal changed(int percent)

    // --- internals ---
    property string mode: ""            // "read" | "step" while a command is in flight
    property bool gotStarted: false
    property bool settled: true
    property bool warned: false
    property string buf: ""
    readonly property bool busy: proc.running || !settled

    function step(dir) {
        if (!available || busy)
            return;
        run("step", Q.brightnessArgs(dir));
    }

    // Absolute percent (the quick-settings slider). Same guard as step(): returns false, and
    // runs nothing, while a command is in flight or with no backlight device.
    function set(percent) {
        var argv = U.brightnessSetArgs(percent);
        if (!available || busy || argv === null)
            return false;
        run("set", argv);
        return true;
    }

    function run(m, argv) {
        mode = m;
        buf = "";
        gotStarted = false;
        settled = false;
        proc.command = argv;
        proc.running = true;
    }

    function finish(text) {
        if (settled)
            return;
        settled = true;
        var r = Q.brightnessParse(text);
        if (mode === "read") {
            available = r !== null;
            if (r !== null)
                percent = r.percent;
        } else if (r !== null) {
            percent = r.percent;
            changed(r.percent);
        }
        mode = "";
    }

    property Process proc: Process {
        // Lines are buffered per run (cleared in run()), so one step never parses another's output.
        stdout: SplitParser {
            onRead: data => root.buf += data + "\n"
        }
        onStarted: root.gotStarted = true
        onRunningChanged: {
            if (running || root.settled)
                return;
            if (root.gotStarted) {
                // Exited: parse on the next turn so the last line has been delivered.
                Qt.callLater(function () { root.finish(root.buf); });
                return;
            }
            // A command that fails to start (brightnessctl not installed) emits neither started
            // nor exited, only runningChanged.
            root.settled = true;
            root.mode = "";
            root.available = false;
            if (!root.warned) {
                root.warned = true;
                console.log("uq: brightnessctl unavailable; brightness disabled");
            }
        }
    }

    Component.onCompleted: run("read", Q.BRIGHTNESS_READ)
}
