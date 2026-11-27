import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets
import QtQuick.Effects
import "../../core"
import "../../services"
import ".."
import "../../logic/security.js" as Sec

// Security status (`security`), from oysterAV when it is installed (`oyst-cli status summary
// --deep --json`: ok, warning or threat, with the reasons). Icon: oysterAV's own icon (a coloured
// ring when it isn't installed), green, yellow or red by the state; oysterAV's tray icon stays out
// of the Tray island while this island is on the bar. Small, Medium and Large add the word. A
// click lists why and opens oysterAV's Security page in Settings. Until oysterAV's first-time
// setup has run (the summary says `setup_completed: false`) the island is red and reads "Set up",
// and a click opens the Setup tab of that page instead of the popup. The reading and its
// notifications (setup needed; the state got worse) are services/SecurityStatus.qml, shared by
// every screen's bar, so a second screen sends no second notification. While a scan runs oysterAV's icon takes the accent colour and
// pulses (steady under Reduce motion), the word reads "Scanning 42%" and the popup shows a progress
// bar. Hidden while oysterAV isn't installed or can't answer. Everything the
// summary says is plain text and cut to length.
Island {
    id: root

    // oysterAV's state, read once for every screen (services/SecurityStatus.qml).
    readonly property var summary: SecurityStatus.summary
    readonly property bool failed: SecurityStatus.failed
    readonly property bool setupNeeded: SecurityStatus.setupNeeded
    readonly property string state: SecurityStatus.state
    readonly property var reasons: SecurityStatus.reasons
    readonly property var scan: SecurityStatus.scan
    readonly property bool scanning: SecurityStatus.scanning
    readonly property string word: setupNeeded ? "Set up"
        : scanning ? Sec.scanWord(scan)
        : state === "ok" ? "Protected"
        : (state === "threat" ? "Threat found" : (state === "warning" ? "Needs attention" : "Security"))
    readonly property color tint: setupNeeded ? Theme.error
        : scanning ? Theme.accent
        : state === "ok" ? Theme.success
        : (state === "threat" ? Theme.error : (state === "warning" ? Theme.warning : Theme.textMuted))

    shown: summary !== null && !failed
    Accessible.role: Accessible.Button
    Accessible.name: setupNeeded ? "Security: first-time setup needed. Open setup"
        : "Security: " + word + ". Show details"

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
                    SecurityStatus.refreshSoon();
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
