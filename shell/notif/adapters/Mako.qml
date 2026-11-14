import QtQuick
import "../../logic/notif.js" as N

// mako through makoctl (spec §2.5). Detected by `makoctl --help`; reads `makoctl list` and
// `makoctl mode`; DnD is the do-not-disturb mode; mako has no centre. Introspecting
// /fr/emersion/Mako says which of those it signals (the Modes property, mako 1.10.0; the
// Notifications property, 1.11.0): those are re-read on a PropertiesChanged seen by a `gdbus
// monitor`, the rest (everything, while the monitor is down) every 5 s. Runs nothing while
// `active` is false. Output is untrusted: notif.js parses and cleans it.
QtObject {
    id: root

    // --- interface ---
    property bool active: false                     // set by NotifBackend
    property var detected: null                     // true | false | null (not checked yet)
    readonly property bool available: active && detected === true
    property int count: 0
    property var latest: null
    property bool dnd: false
    readonly property bool hasCentre: false

    function toggleCentre() {
    }

    function setDnd(on) {
        if (!available)
            return;
        runner.run(["makoctl", "mode", on ? "-a" : "-r", "do-not-disturb"], function () {
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
            runner.run(["makoctl", "list"], function (code, text) {
                if (code !== 0)
                    return;
                var r = N.parseMakoList(text);
                root.count = r.count;
                root.latest = r.latest;
            });
        if (!need || need.dnd)
            runner.run(["makoctl", "mode"], function (code, text) {
                if (code === 0)
                    root.dnd = N.parseMakoModes(text);
            });
    }

    // Asks the running daemon which properties it has; without gdbus or an answer, everything is
    // polled.
    function probe() {
        if (!available)
            return;
        runner.run(["gdbus", "call", "--session", "--dest", "org.freedesktop.Notifications",
                "--object-path", "/fr/emersion/Mako",
                "--method", "org.freedesktop.DBus.Introspectable.Introspect"], function (code, text) {
            if (root.available)
                root.events = code === 0 ? N.makoEvents(text) : N.NO_EVENTS;
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
            runner.run(["makoctl", "--help"], function (code) {
                if (root.active)
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
        path: "/fr/emersion/Mako"
        iface: "fr.emersion.Mako"
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
