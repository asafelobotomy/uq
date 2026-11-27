pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "../core"
import "../logic/security.js" as Sec

// oysterAV's state for the Security island (`oyst-cli status summary --deep --json`: ok, warning or
// threat, with the reasons), read once for every screen's bar: one poll and one notification per
// change, however many screens show the island. It polls only while a Security island is on the
// bar: every 30 s, every 3 s while a scan runs, every 15 s while setup is pending. While oyst-cli
// isn't installed it looks again every 5 minutes.
//
// Notifications (`notify-send`): one when the state gets worse than the last one sent, and one
// when first-time setup is needed. If the notification server isn't up yet (it takes its name a
// few seconds after the shell starts) it tries again, every 10 s, up to 6 times.
Singleton {
    id: root

    readonly property bool wanted: BarLayout.placed("security")
    property var summary: null
    property bool failed: false
    // When oyst-cli last failed to start (not installed): looked for again after missingMs.
    property double missingAt: 0
    readonly property int missingMs: 300000
    // The worst state a notification has gone out for.
    property string notified: "ok"
    // Whether the "set up oysterAV" notification has gone out.
    property bool setupNotified: false
    property int retries: 0
    // shell.bar.options.security.notify: off, the island only shows the state.
    readonly property bool notify: (BarLayout.options.security || ({})).notify !== false

    readonly property bool setupNeeded: Sec.setupNeeded(summary)
    readonly property string state: summary ? String(summary.state) : ""
    readonly property var reasons: summary && Array.isArray(summary.reasons)
        ? summary.reasons.slice(0, 6).map(function (r) { return String(r).slice(0, 120); }) : []
    // The running scan (security.js progress): null when none runs.
    readonly property var scan: Sec.progress(summary)
    readonly property bool scanning: scan !== null && !setupNeeded

    // Once setup has run, a later need for it is news again.
    onSetupNeededChanged: {
        if (!setupNeeded)
            setupNotified = false;
    }
    onWantedChanged: if (wanted) refresh()

    function rank(s) {
        return s === "threat" ? 2 : (s === "warning" ? 1 : 0);
    }

    // One notification when the state gets worse than the last one sent; fixed wording, reasons
    // cleaned to plain words so they can't look like options.
    function maybeNotify() {
        if (!notify)
            return;
        if (setupNeeded) {
            if (setupNotified || notifier.running)
                return;
            notifier.command = ["notify-send", "--app-name=oysterAV", "--icon=security-low",
                "--urgency", "normal", "Security: setup needed",
                "oysterAV's first-time setup hasn't been run. Click the Security island to run it."];
            notifier.target = "setup";
            notifier.running = true;
            return;
        }
        if (state === "" || rank(state) <= rank(notified)) {
            if (state !== "" && rank(state) < rank(notified))
                notified = state;
            return;
        }
        if (notifier.running)
            return;
        var body = reasons.join(". ").replace(/[^A-Za-z0-9 ,.'()\/-]/g, " ").slice(0, 200);
        if (!/^[A-Za-z0-9]/.test(body))
            body = "Open Settings, Security for details.";
        notifier.command = ["notify-send", "--app-name=oysterAV", "--icon=security-low",
            "--urgency", state === "threat" ? "critical" : "normal",
            state === "threat" ? "Security: threat found" : "Security: needs attention", body];
        notifier.target = state;
        notifier.running = true;
    }

    property Process notifier: Process {
        property string target: "ok"
        onExited: function (code) {
            if (code === 0) {
                root.retries = 0;
                if (target === "setup")
                    root.setupNotified = true;
                else
                    root.notified = target;
            } else if (root.retries < 6) {
                root.retries += 1;
                root.retry.restart();
            }
        }
    }
    property Timer retry: Timer {
        interval: 10000
        onTriggered: root.maybeNotify()
    }

    property Process poll: Process {
        id: poll
        property bool didStart: false
        command: ["oyst-cli", "status", "summary", "--deep", "--json"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    var d = JSON.parse(text);
                    if (Sec.readable(d)) {
                        root.summary = d;
                        root.failed = false;
                        root.maybeNotify();
                        return;
                    }
                } catch (e) {
                }
                root.failed = true;
            }
        }
        onStarted: didStart = true
        // oyst-cli failing hides the island; not installed, it is looked for again in 5 minutes.
        onExited: function (code) {
            if (code !== 0)
                root.failed = true;
        }
        onRunningChanged: {
            if (running)
                return;
            if (!didStart) {
                root.failed = true;
                root.missingAt = Date.now();
            }
            didStart = false;
        }
    }

    function refresh() {
        if (!wanted || poll.running)
            return;
        if (missingAt > 0 && Date.now() - missingAt < missingMs)
            return;
        missingAt = 0;
        poll.running = true;
    }
    // Ask again soon (a scan started from the popup shows up in the summary a moment later).
    function refreshSoon() {
        afterScan.restart();
    }

    Component.onCompleted: refresh()
    property Timer ticker: Timer {
        interval: root.scanning ? 3000 : (root.setupNeeded ? 15000 : 30000)
        running: root.wanted
        repeat: true
        onTriggered: root.refresh()
    }
    property Timer afterScan: Timer {
        interval: 4000
        onTriggered: root.refresh()
    }
}
