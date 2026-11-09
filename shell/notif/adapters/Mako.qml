import QtQuick
import "../../logic/notif.js" as N

// mako through makoctl (spec §2.5). Detected by `makoctl --help`; polls `makoctl list` and
// `makoctl mode` every 5 s; DnD is the do-not-disturb mode; mako has no centre. Runs nothing
// while `active` is false. Output is untrusted: notif.js parses and cleans it.
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
    function poll() {
        if (!available)
            return;
        runner.run(["makoctl", "list"], function (code, text) {
            if (code !== 0)
                return;
            var r = N.parseMakoList(text);
            root.count = r.count;
            root.latest = r.latest;
        });
        runner.run(["makoctl", "mode"], function (code, text) {
            if (code === 0)
                root.dnd = N.parseMakoModes(text);
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
            runner.run(["makoctl", "--help"], function (code) {
                if (root.active)
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
