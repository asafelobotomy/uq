.pragma library

// Pure logic for the quick-settings panel and the OSD. No Quickshell imports; the QML layer
// maps live objects to the plain shapes documented below.

var LAYOUTS = ["dwindle", "master", "scrolling"];
// -e4 matches brightnessArgs, so the read and the steps report percent on the same scale.
var BRIGHTNESS_READ = ["brightnessctl", "-m", "-e4"];
var OSD_STARTUP_MS = 2000;
var MAX_WIFI_ROWS = 20;

// One key-repeat step of 0.05, clamped to [0, 1], rounded to 2 dp. Non-finite input -> 0.
function volumeStep(volume, dir) {
    var v = Number(volume);
    if (volume === null || volume === undefined || !isFinite(v))
        return 0;
    var next = v + (dir < 0 ? -0.05 : 0.05);
    next = Math.round(next * 100) / 100;
    return Math.max(0, Math.min(1, next));
}

// Show an OSD only after the startup grace period and only for level changes on the same node
// (a node switch, e.g. unplugged headphones, is not a level change).
function osdShouldShow(startedAtMs, nowMs, prevNodeId, nodeId) {
    // Fails closed: a NaN clock or a missing node never shows an OSD.
    if (!(nowMs - startedAtMs >= OSD_STARTUP_MS))
        return false;
    if (nodeId === null || nodeId === undefined)
        return false;
    return prevNodeId === nodeId;
}

// `brightnessctl -m` lines: device,class,current,percent%,max. First `backlight` line wins.
function brightnessParse(stdout) {
    if (typeof stdout !== "string")
        return null;
    var lines = stdout.split("\n");
    for (var i = 0; i < lines.length; i++) {
        var f = lines[i].replace(/\r$/, "").split(",");
        if (f.length !== 5 || f[1] !== "backlight")
            continue;
        if (!/^\d+$/.test(f[2]) || !/^\d+%$/.test(f[3]) || !/^\d+$/.test(f[4]))
            continue;
        var max = parseInt(f[4], 10);
        if (!(max > 0) || f[0] === "")
            continue;
        return {
            device: f[0],
            current: parseInt(f[2], 10),
            max: max,
            percent: parseInt(f[3], 10)
        };
    }
    return null;
}

function brightnessArgs(dir) {
    return ["brightnessctl", "-m", "-e4", "-n2", "set", dir < 0 ? "5%-" : "5%+"];
}

function _signal(s) {
    var n = Number(s);
    if (!isFinite(n))
        return 0;
    return Math.round(Math.max(0, Math.min(1, n)) * 100);
}

// Input: [{name: string, signal: 0..1 (Quickshell's signalStrength), connected: bool, known: bool, secure: bool}].
// Names are untrusted text; they are passed through untouched for the UI to render as plain
// text. Output rows add `connectable` (a secured unknown network needs a password the shell
// cannot ask for).
function wifiRows(networks) {
    if (!networks)
        return [];
    var rows = [];
    for (var i = 0; i < networks.length; i++) {
        var n = networks[i];
        if (!n || typeof n.name !== "string" || n.name === "")
            continue;
        var connected = !!n.connected, known = !!n.known, secure = !!n.secure;
        rows.push({
            name: n.name,
            signal: _signal(n.signal),
            connected: connected,
            known: known,
            secure: secure,
            connectable: !connected && (known || !secure)
        });
    }
    rows.sort(function (a, b) {
        if (a.connected !== b.connected)
            return a.connected ? -1 : 1;
        if (a.known !== b.known)
            return a.known ? -1 : 1;
        if (a.signal !== b.signal)
            return b.signal - a.signal;
        return a.name < b.name ? -1 : (a.name > b.name ? 1 : 0);
    });
    return rows.slice(0, MAX_WIFI_ROWS);
}

function _hhmm(t) {
    return typeof t === "string" ? t.substring(0, 5) : "";
}

function scheduleLabel(schedule) {
    var m = schedule ? schedule.mode : undefined;
    if (m === "off")
        return "Off";
    if (m === "sun")
        return "Sunset to sunrise";
    if (m === "fixed")
        return "Dark " + _hhmm(schedule.dark_at) + "–" + _hhmm(schedule.light_at);
    return "Unknown";
}

// The Theme group's toggle: off -> sun (needs a location), anything else -> off.
function nextSchedule(schedule, location) {
    var m = schedule ? schedule.mode : undefined;
    if (m === "off") {
        if (!location)
            return null;
        return {mode: "sun", lat: location.lat, lon: location.lon};
    }
    return {mode: "off"};
}

function batteryLabel(percent, charging, secondsLeft) {
    var p = Math.round(Number(percent) || 0);
    if (charging)
        return "Charging " + p + "%";
    var s = Number(secondsLeft);
    if (!(s > 0))
        return p + "%";
    var mins = Math.round(s / 60);
    var h = Math.floor(mins / 60), m = mins % 60;
    var t = h > 0 ? (m > 0 ? h + " h " + m + " min" : h + " h") : mins + " min";
    return p + "% · " + t;
}

// PowerProfile enum order (Quickshell.Services.UPower): PowerSaver=0, Balanced=1, Performance=2.
function profileLabel(p) {
    if (p === 0)
        return "Power saver";
    if (p === 2)
        return "Performance";
    return "Balanced";
}

// OSD bar fill in [0, 1]. A non-positive max or non-finite value -> 0.
function osdFraction(value, max) {
    var v = Number(value), m = Number(max);
    if (!isFinite(v) || !isFinite(m) || !(m > 0))
        return 0;
    return Math.max(0, Math.min(1, v / m));
}

function osdPercent(value, max) {
    return Math.round(osdFraction(value, max) * 100);
}

// Fixed icon-theme names only; an unknown kind -> "" (no icon).
function osdIcon(kind, value, max, muted) {
    var base;
    if (kind === "volume")
        base = "audio-volume-";
    else if (kind === "mic")
        base = "microphone-sensitivity-";
    else if (kind === "brightness")
        return "display-brightness";
    else
        return "";
    var p = osdPercent(value, max);
    if (muted || p === 0)
        return base + "muted";
    if (p < 34)
        return base + "low";
    if (p < 67)
        return base + "medium";
    return base + "high";
}

// The node id to compare level changes against: null while a default-device switch is settling
// or the node isn't bound yet, so osdShouldShow fails closed until then.
function osdSettledId(node, settling) {
    if (settling || !node || !node.ready)
        return null;
    var id = node.id;
    return id === null || id === undefined ? null : id;
}

// Whether an OSD may be drawn while `openSurface` (Surfaces.open) is showing. Quick settings
// shows the levels itself, so its sliders and the keys stay quiet while it is open.
function osdAllowed(openSurface) {
    return openSurface !== "quicksettings";
}
