import QtQuick
import "../../logic/notif.js" as N

// dunst through dunstctl (spec §2.5). Detected by `dunstctl is-paused`; reads `dunstctl count`
// and `dunstctl is-paused`; DnD is `set-paused`; the centre is `history-pop`. The daemon's
// version (GetServerInformation) says which of those it signals (counts since 1.10.0, paused
// since 1.13.1): those are re-read on a PropertiesChanged seen by a `gdbus monitor`, the rest
// (everything, while the monitor is down) every 5 s. Runs nothing while `active` is false.
// Output is untrusted: notif.js parses it.
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
    property var events: N.NO_EVENTS                // what the daemon signals: {dnd, count}
    readonly property var pollNeeds: N.pollNeeds(events, monitor.live)

    // Reads the parts in `need` ({dnd, count}; both when omitted).
    function poll(need) {
        if (!available)
            return;
        if (!need || need.count)
            runner.run(["dunstctl", "count"], function (code, text) {
                if (code === 0)
                    root.count = N.parseDunstCount(text);
            });
        if (!need || need.dnd)
            runner.run(["dunstctl", "is-paused"], function (code, text) {
                if (code === 0)
                    root.dnd = N.parseDunstPaused(text);
            });
    }

    // Asks the running daemon for its version; without gdbus or an answer, everything is polled.
    function probe() {
        if (!available)
            return;
        runner.run(["gdbus", "call", "--session", "--dest", "org.freedesktop.Notifications",
                "--object-path", "/org/freedesktop/Notifications",
                "--method", "org.freedesktop.Notifications.GetServerInformation"], function (code, text) {
            if (root.available)
                root.events = code === 0 ? N.dunstEvents(text) : N.NO_EVENTS;
        });
    }

    function reset() {
        runner.clear();
        detected = null;
        count = 0;
        latest = null;
        dnd = false;
        events = N.NO_EVENTS;
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

    onAvailableChanged: {
        if (available) {
            poll();
            probe();
        }
    }

    property Runner runner: Runner {}

    property DbusMonitor monitor: DbusMonitor {
        wanted: root.available && (root.events.dnd || root.events.count)
        path: "/org/freedesktop/Notifications"
        iface: "org.dunstproject.cmd0"
        // A restarted daemon may be another version: ask again, then read everything.
        onUpdate: appeared => {
            if (appeared)
                root.probe();
            root.poll();
        }
    }

    property Timer pollTimer: Timer {
        interval: 5000
        repeat: true
        running: root.available && (root.pollNeeds.dnd || root.pollNeeds.count)
        onTriggered: {
            if (!root.runner.busy)
                root.poll(root.pollNeeds);
        }
    }
}
