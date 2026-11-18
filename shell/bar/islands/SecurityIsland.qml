import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Widgets
import QtQuick.Effects
import "../../core"
import ".."
import "../../logic/security.js" as Sec

// Security status (`security`), from oysterAV when it is installed (`oyst-cli status summary
// --deep --json`: ok, warning or threat, with the reasons). Icon: oysterAV's own icon (a coloured
// ring when it isn't installed), green, yellow or red by the state; oysterAV's tray icon stays out
// of the Tray island while this island is on the bar. Small, Medium and Large add the word. A
// click lists why and opens oysterAV's Security page in Settings. Until oysterAV's first-time
// setup has run (the summary says `setup_completed: false`) the island is red and reads "Set up",
// sends one notification, and a click opens the Setup tab of that page instead of the popup. When
// the state gets worse it sends one notification (`notify-send`), and tries
// again for a minute if the notification server isn't up yet (it takes its name a few seconds
// after the shell starts). While a scan runs oysterAV's icon takes the accent colour and
// pulses (steady under Reduce motion), the word reads "Scanning 42%" and the popup shows a progress
// bar. Hidden while oysterAV isn't installed or can't answer. Everything the
// summary says is plain text and cut to length.
Island {
    id: root

    property var summary: null
    property bool failed: false
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
    readonly property string word: setupNeeded ? "Set up"
        : scanning ? Sec.scanWord(scan)
        : state === "ok" ? "Protected"
        : (state === "threat" ? "Threat found" : (state === "warning" ? "Needs attention" : "Security"))
    readonly property color tint: setupNeeded ? Theme.error
        : scanning ? Theme.accent
        : state === "ok" ? Theme.success
        : (state === "threat" ? Theme.error : (state === "warning" ? Theme.warning : Theme.textMuted))

    // Once setup has run, a later need for it is news again.
    onSetupNeededChanged: {
        if (!setupNeeded)
            setupNotified = false;
    }

    shown: summary !== null && !failed
    Accessible.role: Accessible.Button
    Accessible.name: setupNeeded ? "Security: first-time setup needed. Open setup"
        : "Security: " + word + ". Show details"

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

    Process {
        id: notifier
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
                retry.restart();
            }
        }
    }
    Timer {
        id: retry
        interval: 10000
        onTriggered: root.maybeNotify()
    }

    Process {
        id: poll
        command: ["oyst-cli", "status", "summary", "--deep", "--json"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    var d = JSON.parse(text);
                    if (d && typeof d.state === "string") {
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
        // oyst-cli missing (exit 127 / not startable) hides the island.
        onExited: function (code) {
            if (code !== 0)
                root.failed = true;
        }
    }

    function refresh() {
        if (!poll.running)
            poll.running = true;
    }
    Component.onCompleted: refresh()
    // Every 30 s; every 3 s while a scan runs, so the bar follows it, every 15 s while setup is
    // pending, so the island turns as soon as it has been run.
    Timer {
        interval: root.scanning ? 3000 : (root.setupNeeded ? 15000 : 30000)
        running: true
        repeat: true
        onTriggered: root.refresh()
    }
    // A scan started from the popup shows up in the summary a moment later.
    Timer {
        id: afterScan
        interval: 4000
        onTriggered: root.refresh()
    }

    RowLayout {
        anchors.centerIn: parent
        width: Math.min(implicitWidth, root.room)
        spacing: Theme.gapIn
        // oysterAV's own icon, shown in its colours' place: green, yellow or red by the status
        // (the same tint as the ring). Without the icon installed, a ring of that colour.
        Item {
            Layout.preferredWidth: 20
            Layout.preferredHeight: 20
            // While a scan runs the icon takes the accent colour and pulses (steady under Reduce motion).
            SequentialAnimation on opacity {
                running: root.scanning && !Settings.reduceMotion && root.visible
                loops: Animation.Infinite
                NumberAnimation { from: 1; to: 0.35; duration: Motion.base * 8; easing.type: Easing.InOutSine }
                NumberAnimation { from: 0.35; to: 1; duration: Motion.base * 8; easing.type: Easing.InOutSine }
            }
            IconImage {
                id: appIcon
                anchors.fill: parent
                visible: false
                implicitSize: 20
                // The icon theme's oysterav, else the file oysterAV installs into hicolor itself, so the
                // island shows it on a desktop whose icon theme lookup misses it.
                source: Quickshell.iconPath("oysterav", true) || "file:///usr/share/icons/hicolor/128x128/apps/oysterav.png"
            }
            MultiEffect {
                anchors.fill: parent
                visible: appIcon.status === Image.Ready
                source: appIcon
                colorization: 0.9
                colorizationColor: root.tint
            }
            Rectangle {
                anchors.centerIn: parent
                visible: appIcon.status !== Image.Ready
                width: 16
                height: 16
                radius: 8
                color: "transparent"
                border.width: 2
                border.color: root.tint
                Rectangle {
                    anchors.centerIn: parent
                    width: 8
                    height: 8
                    radius: 4
                    color: root.tint
                }
            }
        }
        UqText {
            visible: !root.isIcon
            text: root.word
            Layout.fillWidth: true
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: {
            if (root.setupNeeded) {
                popup.hide();
                root.openSetup();
            } else {
                popup.open && popup.anchorItem === root ? popup.hide() : popup.show(root, details);
            }
        }
    }

    LazyPopup {
        id: popup
    }

    // The Setup tab of oysterAV's Security page: the first-time setup, run from Settings.
    function openSetup() {
        Quickshell.execDetached(["uq-settings", "--page", "oysterav-security", "--setting", "setup"]);
    }

    function tintOf(st) {
        return st === "ok" ? Theme.success : (st === "threat" ? Theme.error
            : (st === "warning" ? Theme.warning : Theme.textMuted));
    }

    Component {
        id: details
        ColumnLayout {
            spacing: Theme.gapIn
            width: Theme.fontSize * 26

            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: Theme.gapIn
                Rectangle {
                    Layout.preferredWidth: 10
                    Layout.preferredHeight: 10
                    radius: 5
                    color: root.tint
                }
                UqText {
                    text: root.word
                    baseBold: true
                }
            }
            UqText {
                visible: root.reasons.length === 0
                text: "Everything oysterAV checks is fine."
                color: Theme.textMuted
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
            }
            Repeater {
                model: root.reasons
                delegate: UqText {
                    required property string modelData
                    text: modelData
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                }
            }

            // The running scan: a flat bar with how far it has got and which scanner is at work.
            ColumnLayout {
                Layout.fillWidth: true
                visible: root.scanning
                spacing: Theme.gapIn
                Rectangle {
                    Layout.fillWidth: true
                    visible: root.scan !== null && root.scan.known
                    height: 6
                    radius: 3
                    color: Theme.surfaceAlt
                    Rectangle {
                        objectName: "scanFill"
                        height: parent.height
                        radius: parent.radius
                        color: Theme.accent
                        width: parent.width * (root.scan && root.scan.known ? root.scan.percent / 100 : 0)
                        Behavior on width { NumberAnimation { duration: Motion.base; easing.type: Motion.easing } }
                    }
                }
                UqText {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    color: Theme.textMuted
                    text: root.scan === null ? "" : (root.scan.known ? root.scan.percent + "%" : "Scan running\u2026")
                        + (root.scan !== null && root.scan.pack !== "" ? " \u00b7 " + root.scan.pack : "")
                }
            }

            Rectangle { Layout.fillWidth: true; height: 1; color: Theme.border }

            Repeater {
                model: Sec.facts(root.summary)
                delegate: RowLayout {
                    required property var modelData
                    Layout.alignment: Qt.AlignHCenter
                    spacing: Theme.gapIn
                    UqText {
                        text: modelData.label
                        color: Theme.textMuted
                    }
                    UqText {
                        text: modelData.value
                        color: root.tintOf(modelData.state === "muted" ? "" : modelData.state)
                        Layout.maximumWidth: Theme.fontSize * 16
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                    }
                }
            }
            Repeater {
                model: Sec.checks(root.summary)
                delegate: RowLayout {
                    required property var modelData
                    Layout.alignment: Qt.AlignHCenter
                    spacing: Theme.gapIn
                    UqText {
                        text: modelData.label
                        color: Theme.textMuted
                    }
                    UqText {
                        text: modelData.value
                        color: root.tintOf(modelData.state === "muted" ? "" : modelData.state)
                        Layout.maximumWidth: Theme.fontSize * 16
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                    }
                }
            }

            Rectangle { Layout.fillWidth: true; height: 1; color: Theme.border }

            MenuRow {
                Layout.fillWidth: true
                text: root.summary && root.summary.scanning === true ? "Scan running\u2026" : "Run a quick scan"
                enabled: !(root.summary && root.summary.scanning === true)
                onActivated: {
                    popup.hide();
                    Quickshell.execDetached(["oyst-cli", "scan", "--profile", "quick"]);
                    afterScan.restart();
                }
                Component.onCompleted: {
                    textItem.horizontalAlignment = Text.AlignHCenter;
                    forceActiveFocus();
                }
            }
            MenuRow {
                Layout.fillWidth: true
                text: "Open oysterAV\u2026"
                onActivated: {
                    popup.hide();
                    Quickshell.execDetached(["oysterav"]);
                }
                Component.onCompleted: textItem.horizontalAlignment = Text.AlignHCenter
            }
            MenuRow {
                Layout.fillWidth: true
                text: "Open Security settings\u2026"
                Component.onCompleted: textItem.horizontalAlignment = Text.AlignHCenter
                onActivated: {
                    popup.hide();
                    Quickshell.execDetached(["uq-settings", "--page", "oysterav-security"]);
                }
            }
        }
    }
}
