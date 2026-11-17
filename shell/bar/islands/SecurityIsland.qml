import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../../core"
import ".."
import "../../logic/security.js" as Sec

// Security status (`security`), from oysterAV when it is installed (`oyst-cli status summary
// --deep --json`: ok, warning or threat, with the reasons). Icon: a coloured ring, green, amber
// or red. Small, Medium and Large add the word. A click lists why and opens oysterAV's Security
// page in Settings. When the state gets worse it sends one notification (`notify-send`), and tries
// again for a minute if the notification server isn't up yet (it takes its name a few seconds
// after the shell starts). Hidden while oysterAV isn't installed or can't answer. Everything the
// summary says is plain text and cut to length.
Island {
    id: root

    property var summary: null
    property bool failed: false
    // The worst state a notification has gone out for.
    property string notified: "ok"
    property int retries: 0

    readonly property string state: summary ? String(summary.state) : ""
    readonly property var reasons: summary && Array.isArray(summary.reasons)
        ? summary.reasons.slice(0, 6).map(function (r) { return String(r).slice(0, 120); }) : []
    readonly property string word: state === "ok" ? "Protected"
        : (state === "threat" ? "Threat found" : (state === "warning" ? "Needs attention" : "Security"))
    readonly property color tint: state === "ok" ? Theme.success
        : (state === "threat" ? Theme.error : (state === "warning" ? Theme.warning : Theme.textMuted))

    shown: summary !== null && !failed
    Accessible.role: Accessible.Button
    Accessible.name: "Security: " + word + ". Show details"

    function rank(s) {
        return s === "threat" ? 2 : (s === "warning" ? 1 : 0);
    }

    // One notification when the state gets worse than the last one sent; fixed wording, reasons
    // cleaned to plain words so they can't look like options.
    function maybeNotify() {
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
    // Every two minutes; every 15 s while a scan runs, so the popup follows it.
    Timer {
        interval: root.summary && root.summary.scanning === true ? 15000 : 120000
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
        Rectangle {
            Layout.preferredWidth: 16
            Layout.preferredHeight: 16
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
        UqText {
            visible: !root.isIcon
            text: root.word
            Layout.fillWidth: true
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: popup.open && popup.anchorItem === root ? popup.hide() : popup.show(root, details)
    }

    LazyPopup {
        id: popup
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
