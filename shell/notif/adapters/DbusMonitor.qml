import QtQuick
import Quickshell.Io
import "../../logic/backoff.js" as Backoff
import "../../logic/notif.js" as N

// A long-running `gdbus monitor` on the notification daemon's `path`, so an adapter can read its
// state when it changes instead of every 5 s. Runs while `wanted`; restarted with backoff when it
// exits. `live` is true while it runs and the daemon owns `name` (gdbus has subscribed to that
// owner's signals); while it is false the adapter polls. Bursts of signals are debounced into one
// `update(appeared)`; appeared is true when the daemon took the name again after this monitor
// first saw it (a restart, so its version may have changed). Output is untrusted: notif.js
// classifies each line.
QtObject {
    id: root

    property bool wanted: false
    property string name: "org.freedesktop.Notifications"
    property string path: ""
    property string iface: ""
    readonly property bool live: proc.running && owned

    signal update(bool appeared)

    // --- internals ---
    property bool owned: false
    property bool seenOwner: false                  // an owner line since this process started
    property bool pendingAppeared: false
    property int attempt: 0
    property int stableMs: 60000
    property int debounceMs: 150

    function handle(line) {
        var kind = N.monitorLine(line, name, path, iface);
        if (kind === "lost") {
            owned = false;
        } else if (kind === "owner") {
            owned = true;
            if (seenOwner)
                pendingAppeared = true;
            seenOwner = true;
            debounce.restart();
        } else if (kind === "change") {
            if (!debounce.running)
                debounce.restart();
        }
    }

    function start() {
        if (wanted && !proc.running) {
            owned = false;
            seenOwner = false;
            proc.running = true;
        }
    }

    function stop() {
        restartTimer.stop();
        stableTimer.stop();
        debounce.stop();
        pendingAppeared = false;
        owned = false;
        proc.running = false;
    }

    // Exited or failed to start while still wanted: back off and start it again.
    function gone() {
        owned = false;
        stableTimer.stop();
        if (!wanted)
            return;
        restartTimer.interval = Backoff.next(attempt);
        attempt++;
        restartTimer.restart();
    }

    onWantedChanged: {
        if (wanted) {
            attempt = 0;
            start();
        } else {
            stop();
        }
    }

    property Process proc: Process {
        // Ends with the shell (setpriv --pdeathsig; see BridgeIo.qml).
        command: ["/usr/bin/setpriv", "--pdeathsig", "TERM", "--", "gdbus", "monitor", "--session", "--dest", root.name, "--object-path", root.path]
        stdout: SplitParser {
            onRead: data => root.handle(data)
        }
        onStarted: {
            root.stableTimer.interval = root.stableMs;
            root.stableTimer.restart();
        }
        // An exit and a failed start (gdbus missing) both end in runningChanged.
        onRunningChanged: {
            if (!running)
                root.gone();
        }
    }

    // The first signal of a burst starts it; later ones ride along (a change is never delayed by
    // more than debounceMs).
    property Timer debounce: Timer {
        interval: root.debounceMs
        onTriggered: {
            var a = root.pendingAppeared;
            root.pendingAppeared = false;
            if (root.live)
                root.update(a);
        }
    }
    property Timer restartTimer: Timer {
        onTriggered: root.start()
    }
    // Resets the backoff once the monitor has stayed up for stableMs.
    property Timer stableTimer: Timer {
        onTriggered: root.attempt = 0
    }
}
