pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "../core"
import "../logic/net.js" as Net

// The VPN the status island shows: NetworkManager's active VPN connection (nmcli) or Tailscale's
// exit node. One copy for every screen's bar, so the tools run once rather than once per screen,
// and the bridge hears NetworkChanged once per change.
//
// NetworkManager is followed by `nmcli monitor`, which prints a line on every change; the active
// connections are read again after one. Only while the monitor can't run is nmcli polled. Tailscale
// has no such stream, so `tailscale status` is polled: every 10 s while tailscaled answers, every
// minute while it doesn't (not running, logged out), and after any network change. A missing tool
// is tried again after 5 minutes.
Singleton {
    id: root

    readonly property int pollMs: 10000
    readonly property int idleMs: 60000
    readonly property int retryMs: 300000

    property var vpnConn: null          // string | null, from nmcli
    property var exitNode: null         // string | null, from tailscale
    // The status island's VPN dropdown: Net.parseNmcliConnection() of the active VPN (read when
    // it changes and when the dropdown opens), and Net.parseTailscale() of the last status.
    property var nmDetails: null
    property var tsDetails: null
    readonly property bool vpnActive: Net.vpnActive(vpnConn, exitNode)
    readonly property string vpnLabel: vpnConn ? String(vpnConn) : (exitNode ? String(exitNode) : "")

    property double nmFailedAt: 0
    property double tsFailedAt: 0
    // When tailscale last answered with an error (tailscaled not running): polled slower then.
    property double tsDownAt: 0
    property double tsLastRun: 0
    property var lastSent: null         // identity string last sent, null = nothing sent
    readonly property string vpnId: Net.vpnIdentity(vpnConn, exitNode)

    function pollNetwork(force) {
        var now = Date.now();
        if ((force || !root.nmMonitor.running) && !root.nm.running && Net.retryDue(nmFailedAt, now, retryMs))
            root.nm.running = true;
        var due = force || !tsDownAt || now - tsLastRun >= idleMs - 500;
        if (due && !root.ts.running && Net.retryDue(tsFailedAt, now, retryMs)) {
            tsLastRun = now;
            root.ts.running = true;
        }
    }
    function sendVpn() {
        var id = vpnId;
        if (!Net.identityChanged(lastSent, id))
            return;
        lastSent = id;
        Bridge.call("NetworkChanged", [{ vpn_active: vpnActive }]).catch(function () {});
    }
    onVpnIdChanged: settle.restart()
    onVpnConnChanged: refreshDetails()

    // Read the active NetworkManager VPN's details again (the dropdown calls this as it opens).
    function refreshDetails() {
        if (!vpnConn) {
            nmDetails = null;
            return;
        }
        if (root.nmDetail.running) {
            root.nmDetail.again = true;
        } else {
            root.nmDetail.name = String(vpnConn);
            root.nmDetail.running = true;
        }
    }

    Component.onCompleted: {
        pollNetwork(true);
        settle.restart();      // once at startup with the current value
    }

    property Timer pollTimer: Timer {
        interval: root.pollMs
        running: true
        repeat: true
        onTriggered: root.pollNetwork(false)
    }
    property Timer settle: Timer {
        interval: 3000
        onTriggered: root.sendVpn()
    }
    // A burst of monitor lines (a connection coming up prints several) is one look.
    property Timer changed: Timer {
        interval: 500
        onTriggered: root.pollNetwork(true)
    }
    property Connections bridge: Connections {
        target: Bridge
        function onConnectedChanged() {
            if (Bridge.connected) {
                root.lastSent = null;
                root.settle.restart();
            }
        }
    }

    // `nmcli monitor`: a line per NetworkManager change. If it can't start or ends, nmcli is
    // polled instead (and the monitor tried again with the retry).
    property double monFailedAt: 0
    property Process nmMonitor: Process {
        // Ends with the shell (setpriv --pdeathsig; see BridgeIo.qml). A missing nmcli is
        // setpriv exiting 127: running still goes false, so the retry below is unchanged.
        command: ["/usr/bin/setpriv", "--pdeathsig", "TERM", "--", "nmcli", "monitor"]
        running: Net.retryDue(root.monFailedAt, Date.now(), root.retryMs)
        stdout: SplitParser {
            onRead: root.changed.restart()
        }
        onRunningChanged: {
            if (!running) {
                root.monFailedAt = Date.now();
                root.monRetry.restart();
            }
        }
    }
    property Timer monRetry: Timer {
        interval: root.retryMs
        onTriggered: root.monFailedAt = 0
    }

    // A failed start (missing tool) emits runningChanged(false) without started(); a non-zero
    // exit is an ordinary result. Only the failed start is cached for 5 minutes by pollNetwork().
    property Process nm: Process {
        id: nm
        property bool didStart: false
        property bool gotExit: false
        property bool gotOut: false
        property int exitCode: 0
        property string outText: ""
        // Exit code and stdout arrive in either order; decide once both are in.
        function conclude() {
            if (!gotExit || !gotOut)
                return;
            gotExit = false;
            gotOut = false;
            var res = Net.finishRun(exitCode, outText);
            root.nmFailedAt = 0;
            // Non-zero exit (NetworkManager not ready, restarting): no VPN until the next look.
            root.vpnConn = res.ok ? Net.vpnName(Net.parseNmcliActive(res.text)) : null;
        }
        command: ["nmcli", "-t", "-f", "NAME,TYPE", "con", "show", "--active"]
        stdout: StdioCollector {
            onStreamFinished: {
                nm.outText = text;
                nm.gotOut = true;
                nm.conclude();
            }
        }
        onStarted: { didStart = true; gotExit = false; gotOut = false; }
        onExited: function (code) {
            exitCode = code;
            gotExit = true;
            conclude();
        }
        onRunningChanged: {
            if (running)
                return;
            if (Net.failedStart(didStart)) {
                root.nmFailedAt = Date.now();
                root.vpnConn = null;
            }
            didStart = false;
        }
    }
    property Process ts: Process {
        id: ts
        property bool didStart: false
        property bool gotExit: false
        property bool gotOut: false
        property int exitCode: 0
        property string outText: ""
        function conclude() {
            if (!gotExit || !gotOut)
                return;
            gotExit = false;
            gotOut = false;
            var res = Net.finishRun(exitCode, outText);
            root.tsFailedAt = 0;
            // Non-zero exit (tailscaled not running, logged out): looked at again in a minute.
            root.tsDownAt = res.ok ? 0 : Date.now();
            var parsed = res.ok ? Net.parseTailscale(res.text) : null;
            root.exitNode = parsed ? parsed.exitNode : null;
            root.tsDetails = parsed && parsed.exitNode ? parsed : null;
        }
        command: ["tailscale", "status", "--json"]
        stdout: StdioCollector {
            onStreamFinished: {
                ts.outText = text;
                ts.gotOut = true;
                ts.conclude();
            }
        }
        onStarted: { didStart = true; gotExit = false; gotOut = false; }
        onExited: function (code) {
            exitCode = code;
            gotExit = true;
            conclude();
        }
        onRunningChanged: {
            if (running)
                return;
            if (Net.failedStart(didStart)) {
                root.tsFailedAt = Date.now();
                root.exitNode = null;
                root.tsDetails = null;
            }
            didStart = false;
        }
    }

    // `nmcli -t connection show id <name>`: the active VPN's type, server, interface and address.
    // The name is one argument (no shell); secrets stay hidden (no --show-secrets).
    property Process nmDetail: Process {
        id: nmDetail
        property bool again: false
        property string name: ""
        command: ["nmcli", "-t", "connection", "show", "id", name]
        stdout: StdioCollector {
            onStreamFinished: {
                if (nmDetail.name === String(root.vpnConn))
                    root.nmDetails = Net.parseNmcliConnection(text);
            }
        }
        onRunningChanged: {
            if (running)
                return;
            if (again) {
                again = false;
                root.refreshDetails();
            }
        }
    }
}
