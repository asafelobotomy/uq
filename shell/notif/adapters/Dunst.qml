import QtQuick
import "../../logic/notif.js" as N

// dunst through dunstctl (spec §2.5). Detected by `dunstctl is-paused`; polls `dunstctl count`
// and `dunstctl is-paused` every 5 s; DnD is `set-paused`; the centre is `history-pop`. Runs
// nothing while `active` is false. Output is untrusted: notif.js parses it.
QtObject {
    id: root

    // --- interface ---
    property bool active: false                     // set by NotifBackend
    property var detected: null                     // true | false | null (not checked yet)
    readonly property bool available: active && detected === true
    property int count: 0
    property var latest: null                       // dunstctl gives no latest
    property bool dnd: false
    readonly property bool hasCentre: true

    function toggleCentre() {
        if (!available)
            return;
        runner.run(["dunstctl", "history-pop"], function () {
            root.poll();
        });
    }

    function setDnd(on) {
        if (!available)
            return;
        runner.run(["dunstctl", "set-paused", on ? "true" : "false"], function () {
            root.poll();
        });
    }

    // --- internals ---
    function poll() {
        if (!available)
            return;
        runner.run(["dunstctl", "count"], function (code, text) {
            if (code === 0)
                root.count = N.parseDunstCount(text);
        });
        runner.run(["dunstctl", "is-paused"], function (code, text) {
            if (code === 0)
                root.dnd = N.parseDunstPaused(text);
        });
    }

    function reset() {
        runner.clear();
        detected = null;
        count = 0;
        latest = null;
        dnd = false;
    }

    onActiveChanged: {
        reset();
        if (active)
            runner.run(["dunstctl", "is-paused"], function (code, text) {
                if (!root.active)
                    return;
                if (code === 0)
                    root.dnd = N.parseDunstPaused(text);
                root.detected = code === 0;
            });
    }

    onAvailableChanged: if (available) poll()

    property Runner runner: Runner {}

    property Timer pollTimer: Timer {
        interval: 5000
        repeat: true
        running: root.available
        onTriggered: {
            if (!root.runner.busy)
                root.poll();
        }
    }
}
