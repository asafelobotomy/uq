pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "../logic/backoff.js" as Backoff
import "../logic/notifi.js" as N

// noti-fi control-socket client (spec §2.6). The protocol lives in logic/notifi.js; this owns
// the Socket, the pending call handles and the reconnect backoff. Nothing is written before the
// daemon's hello; a missing socket file just retries quietly.
QtObject {
    id: root

    // --- public state ---
    property bool available: false      // true after a valid hello
    property bool dnd: false

    // --- internals ---
    readonly property string runtimeDir: String(Quickshell.env("XDG_RUNTIME_DIR") || "")
    readonly property string socketPath: runtimeDir === "" ? "" : runtimeDir + "/notifd.sock"
    property var proto: ({ greeted: false, pending: {} })
    property var handles: ({})
    property int nextId: 1
    property int attempt: 0
    property int stableMs: 60000
    // True while a connection attempt or connection is ours to tear down; makes drop() run once.
    property bool live: false

    // Same then/catch shape as Bridge.call's handle.
    function makeCall() {
        var ok = [], bad = [];
        var settled = false, value, failed = false;
        var h = {
            then: function (cb) {
                if (settled && !failed) cb(value); else if (!settled) ok.push(cb);
                return h;
            },
            catch: function (cb) {
                if (settled && failed) cb(value); else if (!settled) bad.push(cb);
                return h;
            },
            _resolve: function (v) {
                if (settled) return;
                settled = true; value = v;
                ok.forEach(function (cb) { cb(v); });
            },
            _reject: function (e) {
                if (settled) return;
                settled = true; failed = true; value = e;
                bad.forEach(function (cb) { cb(e); });
            }
        };
        return h;
    }

    function rejectSoon(msg) {
        var h = makeCall();
        // Fail on the next turn so callers can attach handlers first.
        Qt.callLater(function () { h._reject(msg); });
        return h;
    }

    function send(method, params) {
        if (!available)
            return rejectSoon("noti-fi unavailable");
        var id = nextId++;
        var h = makeCall();
        var hs = handles;
        hs[id] = h;
        handles = hs;
        var st = proto;
        st.pending[id] = method;
        proto = st;
        sock.write(N.request(id, method, params) + "\n");
        sock.flush();
        return h;
    }

    // Shows noti-fi's OSD. Rejects at once (next turn) when unavailable or the params are invalid.
    function osd(kind, value, max, muted) {
        var p = N.osdParams(kind, value, max, muted);
        if (p === null)
            return rejectSoon("bad_params");
        return send("osd", p);
    }

    function setDnd(on) {
        return send("dnd", { mode: on ? "on" : "off" });
    }

    function failPending(msg) {
        var hs = handles;
        handles = ({});
        for (var id in hs)
            hs[id]._reject(msg);
    }

    // Retry step: probe for the socket file first, quietly. Quickshell's Socket logs a warning
    // on every failed connect, so a missing file (noti-fi not installed or not running) must not
    // reach it. Any other probe result (a socket is not a regular file) goes on to connect.
    function attemptConnect() {
        if (socketPath === "")
            return;
        probe.path = socketPath;
        probe.reload();
    }

    function connect() {
        if (live)
            return;
        // Clear any socket left by drop() before reconnecting; `live` is false, so the resulting
        // connectionStateChanged is a no-op.
        sock.connected = false;
        live = true;
        sock.path = socketPath;
        sock.connected = true;
    }

    function scheduleRetry() {
        retryTimer.interval = Backoff.next(attempt);
        attempt++;
        retryTimer.restart();
    }

    // Disconnect, socket error or a non-noti-fi peer: fail everything and retry with backoff.
    function drop() {
        if (!live)
            return;
        live = false;
        stableTimer.stop();
        available = false;
        proto = ({ greeted: false, pending: {} });
        failPending("noti-fi disconnected");
        // Close on the next turn: drop() can run inside the Socket's own signal handlers.
        Qt.callLater(function () {
            if (!root.live)
                sock.connected = false;
        });
        scheduleRetry();
    }

    function feed(line) {
        if (!live)
            return;
        var r = N.handleLine(proto, line);
        proto = r.state;
        var e = r.effect;
        switch (e.kind) {
        case "hello":
            available = true;
            stableTimer.interval = stableMs;
            stableTimer.restart();
            send("subscribe", {});
            break;
        case "unavailable":
            drop();
            break;
        case "state":
            dnd = e.dnd;
            break;
        case "reply":
            var hs = handles;
            var h = hs[e.id];
            delete hs[e.id];
            handles = hs;
            if (e.ok) {
                var d = N.replyDnd(e.method, e.value);
                if (d !== null)
                    dnd = d;
            }
            if (h) {
                if (e.ok) h._resolve(e.value); else h._reject(e.error);
            }
            break;
        }
    }

    property Socket sock: Socket {
        parser: SplitParser {
            onRead: data => root.feed(data)
        }
        onConnectionStateChanged: {
            if (!connected)
                root.drop();
        }
    }

    property FileView probe: FileView {
        preload: false
        printErrors: false
        onLoaded: root.connect()
        onLoadFailed: error => {
            if (error === FileViewError.FileNotFound)
                root.scheduleRetry();
            else
                root.connect();
        }
    }

    property Timer retryTimer: Timer {
        onTriggered: root.attemptConnect()
    }
    // Resets the backoff once a connection has stayed up for stableMs.
    property Timer stableTimer: Timer {
        onTriggered: root.attempt = 0
    }

    Component.onCompleted: {
        // Connected in JS: the handler form needs QLocalSocket::LocalSocketError, which qmllint
        // cannot resolve.
        sock.error.connect(function () { root.drop(); });
        attemptConnect();
    }
}
