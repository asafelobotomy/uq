pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "../core"
import "../logic/crumbs.js" as C

// crumBS's reading for the Storage cleanup island (`crumbs scan --json`: the space that can be
// reclaimed, by category), read once for every screen's bar. It scans only while a Storage
// cleanup island is on the bar: when one is placed, then every half hour, at low priority. The
// scan only reads.
Singleton {
    id: root

    readonly property bool wanted: BarLayout.placed("crumbs")
    // crumbs.js parse(): the categories with something to free, biggest first; null before the
    // first answer.
    property var rows: null
    // crumbs missing (exit 127) or failing hides the island.
    property bool failed: false
    readonly property bool scanning: scan.running

    onWantedChanged: if (wanted) refresh()

    property Process scan: Process {
        id: scan
        command: ["nice", "-n", "19", "crumbs", "scan", "--json"]
        stdout: StdioCollector {
            onStreamFinished: {
                var r = C.parse(text);
                if (r !== null) {
                    root.rows = r;
                    root.failed = false;
                } else {
                    root.failed = true;
                }
            }
        }
        onExited: function (code) {
            if (code !== 0)
                root.failed = true;
        }
    }

    function refresh() {
        if (wanted && !scan.running)
            scan.running = true;
    }
    Component.onCompleted: refresh()
    property Timer ticker: Timer {
        interval: 1800000
        running: root.wanted
        repeat: true
        onTriggered: root.refresh()
    }
}
