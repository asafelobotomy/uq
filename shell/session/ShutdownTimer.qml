pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "../core"
import "../logic/shutdowntimer.js" as T

// "Shut down in…" (quick settings, System tab). One timer for the session: start(minutes) sets
// it, cancel() drops it. Warnings go out as notifications 5 minutes and 1 minute before, each
// with a Cancel button; at 0 it runs the bridge's Power "poweroff", the same call as the power
// menu's Shut down. It fires while the screen is locked too: an unattended machine is the point.
// A deadline slept through (suspend) is dropped, never run on wake. The deadline is kept in
// $XDG_RUNTIME_DIR/uq-shutdown-at so a shell restart keeps it; logging out or rebooting clears it.
QtObject {
    id: root

    // --- public state ---
    property var scheduled: null    // shutdowntimer.js schedule, or null
    property double now: Date.now() // ms, refreshed every second while scheduled
    readonly property bool active: scheduled !== null
    readonly property string countdown: T.countdownLabel(scheduled, now)
    // Follows the 12/24-hour clock setting (Settings > Date and time).
    readonly property string at: T.atLabel(scheduled, !(Settings.values.shell && Settings.values.shell.clock_24h === false))

    readonly property string runtimeDir: String(Quickshell.env("XDG_RUNTIME_DIR") || "")
    readonly property string statePath: runtimeDir !== "" ? runtimeDir + "/uq-shutdown-at" : ""
    property bool restored: false

    // Returns true when the timer was set (minutes in 1 to 1440).
    function start(minutes) {
        var t = Date.now();
        var s = T.schedule(minutes, t);
        if (s === null)
            return false;
        now = t;
        scheduled = s;
        save();
        return true;
    }

    function cancel() {
        if (scheduled === null)
            return;
        scheduled = null;
        save();
    }

    function save() {
        if (statePath !== "")
            file.setText(T.serialize(scheduled));
    }

    function tick() {
        now = Date.now();
        var s = scheduled;
        if (s === null)
            return;
        if (T.missed(s, now)) {
            cancel();
            notify(["Scheduled shutdown skipped", "The computer was asleep when it was due."], false);
            return;
        }
        if (T.due(s, now)) {
            cancel();
            Bridge.call("Power", ["poweroff"]).catch(function (e) {
                console.warn("uq[POWER-001] scheduled shutdown failed: " + e);
                root.notify(["Couldn't shut down", "The scheduled shutdown didn't run."], false);
            });
            return;
        }
        var w = T.warningDue(s, now);
        if (w > 0) {
            scheduled = T.markWarned(s, w);
            notify(T.warningText(T.remaining(s, now)), true);
        }
    }

    // A notification; `withCancel` adds a "Cancel shutdown" button that cancels the timer.
    function notify(text, withCancel) {
        if (notifier.running)
            notifier.running = false;
        var cmd = ["notify-send", "--app-name=uQ", "--icon=system-shutdown",
            "--urgency", withCancel ? "critical" : "normal"];
        if (withCancel)
            cmd.push("--action=cancel=Cancel shutdown");
        notifier.command = cmd.concat([text[0], text[1]]);
        // A Process stopped and started in the same turn doesn't restart.
        Qt.callLater(function () { notifier.running = true; });
    }

    property Timer ticker: Timer {
        interval: 1000
        repeat: true
        running: root.scheduled !== null
        onTriggered: root.tick()
    }

    // notify-send with an action waits and prints the action's key when it is clicked.
    property Process notifier: Process {
        stdout: SplitParser {
            onRead: function (line) {
                if (line.trim() === "cancel")
                    root.cancel();
            }
        }
    }

    property FileView file: FileView {
        path: root.statePath
        printErrors: false
        atomicWrites: true
        onLoaded: {
            if (root.restored)
                return;
            root.restored = true;
            var s = T.restore(text(), Date.now());
            if (s !== null && root.scheduled === null) {
                root.now = Date.now();
                root.scheduled = s;
            }
        }
        onLoadFailed: root.restored = true
    }
}
