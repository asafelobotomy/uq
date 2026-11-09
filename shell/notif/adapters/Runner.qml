import QtQuick
import Quickshell.Io

// Runs an adapter's short daemon commands one at a time, each a fixed argv (never a shell).
// run(argv, cb) queues; cb(code, text) gets the exit code (-1 when the command failed to start or
// timed out) and its stdout, capped at MAX_OUT. clear() drops the queue and kills the
// command in flight without calling its callback.
QtObject {
    id: root

    readonly property int maxQueue: 8
    readonly property int maxOut: 65536
    readonly property bool busy: current !== null || queue.length > 0

    // --- internals ---
    property var queue: []
    property var current: null          // {argv, cb} in flight, or null
    property bool didStart: false
    property bool gotExit: false
    property bool timedOut: false
    property int exitCode: -1
    property string outText: ""

    function run(argv, cb) {
        if (queue.length >= maxQueue)
            return;
        queue = queue.concat([{ argv: argv, cb: cb }]);
        if (current === null)
            next();
    }

    function clear() {
        queue = [];
        if (current !== null) {
            current.cb = null;
            proc.running = false;
        }
    }

    function next() {
        if (current !== null || queue.length === 0)
            return;
        current = queue[0];
        queue = queue.slice(1);
        didStart = false;
        gotExit = false;
        timedOut = false;
        exitCode = -1;
        outText = "";
        proc.command = current.argv;
        limit.restart();
        proc.running = true;
    }

    function finish() {
        var c = current;
        if (c === null)
            return;
        limit.stop();
        current = null;
        var code = didStart && gotExit && !timedOut ? exitCode : -1;
        if (c.cb)
            c.cb(code, outText);
        next();
    }

    property Process proc: Process {
        stdout: SplitParser {
            onRead: data => {
                if (root.outText.length < root.maxOut)
                    root.outText += data + "\n";
            }
        }
        onStarted: root.didStart = true
        // A command that fails to start (not installed) emits only runningChanged. Settle on the
        // next turn so the exit code and the last stdout line have both arrived.
        onRunningChanged: {
            if (!running && root.current !== null)
                Qt.callLater(root.finish);
        }
    }

    // Connected in JS: the handler form needs QProcess::ExitStatus, which qmllint cannot resolve.
    // A crash reports the signal number, so it is never 0.
    Component.onCompleted: {
        proc.exited.connect(function (code) {
            root.exitCode = code;
            root.gotExit = true;
        });
    }

    // A wedged daemon client must not hold the queue.
    property Timer limit: Timer {
        interval: 5000
        onTriggered: {
            root.timedOut = true;
            root.proc.running = false;
        }
    }
}
