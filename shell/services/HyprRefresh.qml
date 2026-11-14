pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland

// One place that asks Hyprland for fresh lists. The bar's islands (on every screen), the edge
// indicators, ColumnPolicy and FitWindows all want fresh windows after the same events; each
// asking on its own timer cost several j/clients round trips per focus change, and each reply
// re-ran every consumer's rebuild. Here the asks of one burst become one request per list.
//
// request(): want fresh windows (and, with {monitors: true} / {workspaces: true}, those lists
// too). The first ask starts a short window that the rest of the burst joins; it is not pushed
// back by later asks, so a stream of events (a title ticking) still refreshes every `delay` ms.
// `sent` fires as the requests go out; `settled` a moment later, once lastIpcObject has had time
// to arrive, for consumers that read the lists then rather than watching them.
Singleton {
    id: root

    readonly property int delay: 50
    readonly property int settleMs: 150

    signal sent()
    signal settled()

    property bool wantToplevels: false
    property bool wantMonitors: false
    property bool wantWorkspaces: false

    function request(what) {
        root.wantToplevels = true;
        if (what && what.monitors)
            root.wantMonitors = true;
        if (what && what.workspaces)
            root.wantWorkspaces = true;
        if (!sendTimer.running)
            sendTimer.start();
    }

    property Timer sendTimer: Timer {
        interval: root.delay
        onTriggered: {
            if (root.wantWorkspaces)
                Hyprland.refreshWorkspaces();
            if (root.wantMonitors)
                Hyprland.refreshMonitors();
            if (root.wantToplevels)
                Hyprland.refreshToplevels();
            root.wantToplevels = root.wantMonitors = root.wantWorkspaces = false;
            root.sent();
            // Like the send, the settle is not pushed back by a stream of sends: it fires once
            // more after the last of them instead.
            if (settleTimer.running)
                root.sentDuringSettle = true;
            else
                settleTimer.start();
        }
    }
    property bool sentDuringSettle: false
    property Timer settleTimer: Timer {
        interval: root.settleMs
        onTriggered: {
            root.settled();
            if (root.sentDuringSettle) {
                root.sentDuringSettle = false;
                settleTimer.start();
            }
        }
    }
}
