import QtQuick
import Quickshell.Io

// Runs an adapter's short daemon commands one at a time, each a fixed argv (never a shell).
// run(argv, cb) queues; cb(code, text) gets the exit code (-1 when the command failed to start or
// timed out) and its stdout, capped at maxOut. clear() drops the queue and stops the command in
// flight without calling its callback. Each command gets its own Process, so one that ignores
// termination is killed and abandoned instead of holding the queue.
QtObject {
    id: root

    readonly property int maxQueue: 8
    readonly property int maxOut: 65536
    readonly property bool busy: current !== null || queue.length > 0

    // --- internals ---
    property var queue: []
    // The entry in flight, or null: {argv, cb, proc, didStart, gotExit, code, out, timedOut, done}.
    property var current: null

    function run(argv, cb) {
        if (queue.length >= maxQueue)
            return;
        queue = queue.concat([{ argv: argv, cb: cb }]);
        if (current === null)
            next();
    }

    function clear() {
        queue = [];
        var c = current;
        if (c !== null) {
            c.cb = null;
            if (c.proc)
                c.proc.running = false;
        }
    }

    function next() {
        if (current !== null || queue.length === 0)
            return;
        var c = queue[0];
        queue = queue.slice(1);
        c.didStart = false;
        c.gotExit = false;
        c.code = -1;
        c.out = "";
        c.timedOut = false;
        c.done = false;
        current = c;
        var p = procComp.createObject(root, { command: c.argv });
        c.proc = p;
        // Every handler is a closure over this entry (an object passed through createObject
        // would be copied). exited is connected here anyway: the handler form needs
        // QProcess::ExitStatus, which qmllint cannot resolve. A crash reports the signal number,
        // so it is never 0.
        p.stdout.read.connect(function (data) { root.append(c, data); });
        p.started.connect(function () { c.didStart = true; });
        p.exited.connect(function (code) {
            c.code = code;
            c.gotExit = true;
        });
        // A command that fails to start (not installed) emits only runningChanged. Settle on the
        // next turn so the exit code and the last stdout line have both arrived.
        p.runningChanged.connect(function () {
            if (!p.running)
                Qt.callLater(function () { root.finish(c); });
        });
        limit.restart();
        p.running = true;
    }

    // Each stdout line is cut to what is left of the maxOut budget.
    function append(c, data) {
        if (c.done)
            return;
        var room = maxOut - c.out.length;
        if (room <= 0)
            return;
        var line = data + "\n";
        c.out += line.length > room ? line.slice(0, room) : line;
    }

    function finish(c) {
        if (!c || c.done)
            return;
        c.done = true;
        if (current === c) {
            limit.stop();
            kill.stop();
            current = null;
        }
        var p = c.proc;
        c.proc = null;
        if (p) {
            if (p.running)
                p.signal(9);
            p.destroy();
        }
        var code = c.didStart && c.gotExit && !c.timedOut ? c.code : -1;
        if (c.cb) {
            try {
                c.cb(code, c.out);
            } catch (e) {
                console.warn("uq: notification command callback failed: " + e);
            }
        }
        next();
    }

    property Component procComp: Component {
        Process {
            stdout: SplitParser {}
        }
    }

    // A wedged daemon client must not hold the queue: terminate it after 5 s, and if it is still
    // there a second later, kill it and move on with exit -1.
    property Timer limit: Timer {
        interval: 5000
        onTriggered: {
            var c = root.current;
            if (c === null)
                return;
            c.timedOut = true;
            if (c.proc)
                c.proc.running = false;
            root.kill.restart();
        }
    }
    property Timer kill: Timer {
        interval: 1000
        onTriggered: root.finish(root.current)
    }
}
