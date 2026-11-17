import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../../core"
import ".."

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
    // shell.bar.options.security.notify: off, the island only shows the state.
    readonly property bool notify: (BarLayout.options.security || ({})).notify !== false

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
        if (!notify)
            return;
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
    Timer {
        interval: 120000
        running: true
        repeat: true
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

    Component {
        id: details
        ColumnLayout {
            spacing: Theme.gapIn
            width: Theme.fontSize * 24

            UqText {
                text: root.word
                baseBold: true
            }
            UqText {
                visible: root.reasons.length === 0
                text: "No open findings; signatures and checks are fine."
                color: Theme.textMuted
                Layout.fillWidth: true
                wrapMode: Text.Wrap
            }
            Repeater {
                model: root.reasons
                delegate: UqText {
                    required property string modelData
                    text: modelData
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                }
            }
            MenuRow {
                Layout.fillWidth: true
                text: "Open Security settings…"
                onActivated: {
                    popup.hide();
                    Quickshell.execDetached(["uq-settings", "--page", "oysterav-security"]);
                }
                Component.onCompleted: forceActiveFocus()
            }
        }
    }
}
