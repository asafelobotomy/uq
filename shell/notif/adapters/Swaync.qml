import QtQuick
import Quickshell.Io
import "../../logic/backoff.js" as Backoff
import "../../logic/notif.js" as N

// SwayNotificationCenter through swaync-client (spec §2.5). Detected by `swaync-client
// --version`; count and DnD are pushed by a long-running `swaync-client -swb`, restarted with
// backoff when it exits; the centre is `-t`, DnD `-dn` / `-df`. Runs nothing while `active` is
// false. Output is untrusted: notif.js parses it.
QtObject {
    id: root

    // --- interface ---
    property bool active: false                     // set by NotifBackend
    property var detected: null                     // true | false | null (not checked yet)
    readonly property bool available: active && detected === true
    property int count: 0
    property var latest: null                       // swaync-client gives no latest
    property bool dnd: false
    readonly property bool hasCentre: true

    // DnD and the centre's state follow through the -swb subscription.
    function toggleCentre() {
        if (available)
            runner.run(["swaync-client", "-t"], null);
    }

    function setDnd(on) {
        if (available)
            runner.run(["swaync-client", on ? "-dn" : "-df"], null);
    }

    // --- internals ---
    property int attempt: 0
    property int stableMs: 60000

    function apply(line) {
        var r = N.parseSwaync(line);
        if (r === null)
            return;
        count = r.count;
        dnd = r.dnd;
    }

    function reset() {
        runner.clear();
        detected = null;
        count = 0;
        latest = null;
        dnd = false;
    }

    function stopSubscriber() {
        restartTimer.stop();
        stableTimer.stop();
        sub.running = false;
    }

    function startSubscriber() {
        if (available && !sub.running)
            sub.running = true;
    }

    // The subscriber exited or failed to start while still wanted: back off and start it again.
    function subscriberGone() {
        stableTimer.stop();
        if (!available)
            return;
        restartTimer.interval = Backoff.next(attempt);
        attempt++;
        restartTimer.restart();
    }

    onActiveChanged: {
        reset();
        if (active)
            runner.run(["swaync-client", "--version"], function (code) {
                if (root.active)
                    root.detected = code === 0;
            });
    }

    onAvailableChanged: {
        if (available) {
            attempt = 0;
            startSubscriber();
        } else {
            stopSubscriber();
        }
    }

    property Runner runner: Runner {}

    property Process sub: Process {
        command: ["swaync-client", "-swb"]
        stdout: SplitParser {
            onRead: data => root.apply(data)
        }
        onStarted: {
            root.stableTimer.interval = root.stableMs;
            root.stableTimer.restart();
        }
        // An exit and a failed start (not installed any more) both end in runningChanged.
        onRunningChanged: {
            if (!running)
                root.subscriberGone();
        }
    }

    property Timer restartTimer: Timer {
        onTriggered: root.startSubscriber()
    }
    // Resets the backoff once the subscriber has stayed up for stableMs.
    property Timer stableTimer: Timer {
        onTriggered: root.attempt = 0
    }
}
