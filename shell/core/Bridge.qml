pragma Singleton
import QtQuick
import "../logic/backoff.js" as Backoff

// Protocol side of the `uqctl bridge` link. Pure QML (testable under qmltestrunner); the
// Process lives in BridgeIo.qml, which feeds lines in and reports start/exit.
QtObject {
    id: root

    // --- public state ---
    property bool connected: false      // true after Hello
    property bool uqdUp: false
    property bool locked: false
    property bool sharing: false        // screen sharing (Hyprland screencast), spec §2.10
    property var weather: null          // last Weather event data; null until one arrives / cleared
    property var publicIp: null
    // Owner of org.freedesktop.Notifications (spec 2.3); known stays false until a NotifOwner event.
    property var notifOwner: ({ known: false, owner: null, name: "", self: false })
    property string configError: ""

    signal event(string name, var data)
    signal themeChanged()
    signal settingsChanged(var settings)
    signal layoutChanged(int ws, string layout)
    // Spotlight results: {generation, provider, provider_name, source_dir, items}.
    signal searchResults(var data)

    // --- transport, set by BridgeIo ---
    property var writer: null           // function(line)
    // The Process should be running while this is true.
    property bool processWanted: true

    // --- restart/backoff ---
    property int attempt: 0
    property int restartDelay: 0
    property int stableMs: 60000
    property int nextId: 1
    property var pending: ({})

    function reset() {
        restartTimer.stop();
        stableTimer.stop();
        attempt = 0;
        restartDelay = 0;
        processWanted = true;
        connected = false;
        uqdUp = false;
        sharing = false;
        weather = null;
        publicIp = null;
        notifOwner = ({ known: false, owner: null, name: "", self: false });
        failPending("reset");
    }

    function processStarted() {
        processWanted = true;
        stableTimer.interval = stableMs;
        stableTimer.restart();
    }

    function processExited() {
        // BridgeIo reports both `exited` and a failed start (runningChanged only); count once.
        if (!processWanted)
            return;
        stableTimer.stop();
        connected = false;
        uqdUp = false;
        writer = null;
        failPending("bridge exited");
        restartDelay = Backoff.next(attempt);
        attempt++;
        processWanted = false;
        restartTimer.interval = restartDelay;
        restartTimer.restart();
    }

    function failPending(msg) {
        var p = pending;
        pending = ({});
        for (var id in p)
            p[id]._reject(msg);
    }

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

    // Promise-like: returns {then(cb), catch(cb)}.
    function call(name, args) {
        var h = makeCall();
        if (!writer) {
            // Not attached: fail on the next turn so callers can attach handlers first.
            Qt.callLater(function () { h._reject("bridge not running"); });
            return h;
        }
        var id = nextId++;
        var p = pending;
        p[id] = h;
        pending = p;
        writer(JSON.stringify({ v: 1, id: id, call: name, args: args || [] }));
        return h;
    }

    function feed(line) {
        var m;
        try { m = JSON.parse(line); } catch (e) { return; }
        if (m === null || typeof m !== "object" || m.v !== 1)
            return;
        if (typeof m.event === "string") {
            handleEvent(m.event, m.data === undefined ? null : m.data);
        } else if (m.id !== undefined && pending[m.id]) {
            var h = pending[m.id];
            var p = pending;
            delete p[m.id];
            pending = p;
            if (m.err !== undefined)
                h._reject(String(m.err));
            else
                h._resolve(m.ok === undefined ? null : m.ok);
        }
    }

    function handleEvent(name, data) {
        switch (name) {
        case "Hello":
            uqdUp = !!(data && data.uqd_up);
            locked = !!(data && data.locked);
            connected = true;
            break;
        case "UqdUp": uqdUp = true; break;
        case "UqdDown": uqdUp = false; break;
        case "ThemeChanged": themeChanged(); break;
        case "SettingsChanged": settingsChanged(data); break;
        case "LayoutChanged":
            if (data) layoutChanged(Number(data.workspace), String(data.layout));
            break;
        case "Weather": weather = data; break;
        case "PublicIp": publicIp = data; break;
        case "NotifOwner":
            if (data && typeof data === "object")
                notifOwner = {
                    known: true,
                    owner: data.owner === null || data.owner === undefined ? null : String(data.owner),
                    name: data.name ? String(data.name) : "",
                    self: data.self === true
                };
            break;
        case "SearchResults":
            if (data && typeof data === "object") searchResults(data);
            break;
        case "ConfigError": configError = data ? String(data.message) : ""; break;
        }
        event(name, data);
    }

    // Hyprland rawEvent "screencast": data "<state>,<owner>", state 1 = started.
    function handleRawEvent(name, data) {
        if (name !== "screencast")
            return;
        sharing = String(data).split(",")[0] === "1";
    }

    property Timer restartTimer: Timer {
        onTriggered: root.processWanted = true
    }
    // Resets the backoff once a connection has stayed up for stableMs.
    property Timer stableTimer: Timer {
        onTriggered: root.attempt = 0
    }
}
