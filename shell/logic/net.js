.pragma library

// Parse `nmcli -t -f NAME,TYPE con show --active`. Terse mode escapes ':' and '\' with a
// backslash inside fields. Names are untrusted and returned unchanged.
function parseNmcliActive(text) {
    var out = [];
    if (typeof text !== "string")
        return out;
    var lines = text.split("\n");
    for (var i = 0; i < lines.length; i++) {
        var line = lines[i];
        if (line === "")
            continue;
        var fields = [], cur = "";
        for (var j = 0; j < line.length; j++) {
            var c = line[j];
            if (c === "\\" && j + 1 < line.length) {
                cur += line[j + 1];
                j++;
            } else if (c === ":") {
                fields.push(cur);
                cur = "";
            } else {
                cur += c;
            }
        }
        fields.push(cur);
        if (fields.length < 2)
            continue;
        out.push({ name: fields[0], type: fields[1] });
    }
    return out;
}

// Name of the first active VPN/WireGuard connection, or null.
function vpnName(conns) {
    if (!Array.isArray(conns))
        return null;
    for (var i = 0; i < conns.length; i++) {
        var t = conns[i].type;
        if (t === "vpn" || t === "wireguard")
            return conns[i].name;
    }
    return null;
}

// `tailscale status --json` -> {exitNode: hostname|null}. The exit node is the peer matching
// ExitNodeStatus (by ID, then by Tailscale IP), or a peer flagged ExitNode.
function parseTailscale(json) {
    var none = { exitNode: null };
    var o = json;
    if (typeof json === "string") {
        try { o = JSON.parse(json); } catch (e) { return none; }
    }
    if (!o || typeof o !== "object")
        return none;
    var peers = o.Peer && typeof o.Peer === "object" ? o.Peer : {};
    var keys = Object.keys(peers);
    var es = o.ExitNodeStatus;
    if (es && typeof es === "object") {
        var ips = Array.isArray(es.TailscaleIPs) ? es.TailscaleIPs.map(function (s) {
            return String(s).split("/")[0];
        }) : [];
        for (var i = 0; i < keys.length; i++) {
            var p = peers[keys[i]];
            if (!p || !p.HostName)
                continue;
            var pips = Array.isArray(p.TailscaleIPs) ? p.TailscaleIPs : [];
            var ipHit = pips.some(function (a) { return ips.indexOf(String(a)) >= 0; });
            if ((es.ID && p.ID === es.ID) || ipHit)
                return { exitNode: String(p.HostName) };
        }
    }
    for (var k = 0; k < keys.length; k++) {
        var q = peers[keys[k]];
        if (q && q.ExitNode === true && q.HostName)
            return { exitNode: String(q.HostName) };
    }
    return none;
}

function vpnActive(vpn, exitNode) {
    return !!vpn || !!exitNode;
}

// A tool that failed to start (binary missing) is not retried until `ttl` ms after `failedAt`
// (0 = not failed). A non-zero exit is a normal result and never sets failedAt.
function retryDue(failedAt, now, ttl) {
    return !failedAt || now - failedAt >= ttl;
}

// Sink volume step of +/-5 percentage points, clamped to 0..1.5.
function stepVolume(v, dir) {
    var cur = typeof v === "number" && isFinite(v) ? v : 0;
    var n = Math.round((cur + (dir > 0 ? 0.05 : -0.05)) * 1000) / 1000;
    return Math.max(0, Math.min(1.5, n));
}

function volumeIcon(v, muted) {
    if (muted || !(v > 0))
        return "audio-volume-muted-symbolic";
    if (v < 0.34)
        return "audio-volume-low-symbolic";
    if (v < 0.67)
        return "audio-volume-medium-symbolic";
    return "audio-volume-high-symbolic";
}

// strength 0..1
function wifiIcon(s) {
    if (!(s > 0))
        return "network-wireless-signal-none-symbolic";
    if (s >= 0.8)
        return "network-wireless-signal-excellent-symbolic";
    if (s >= 0.55)
        return "network-wireless-signal-good-symbolic";
    if (s >= 0.3)
        return "network-wireless-signal-ok-symbolic";
    return "network-wireless-signal-weak-symbolic";
}

// UPowerDevice.percentage is a 0..1 fraction.
function batteryPct(p) {
    if (typeof p !== "number" || !isFinite(p))
        return 0;
    return Math.max(0, Math.min(100, Math.round(p * 100)));
}

// Identity of the current VPN: changes when the VPN or exit node changes, not only on/off.
function vpnIdentity(vpn, exitNode) {
    return (vpn ? String(vpn) : "") + "\u0000" + (exitNode ? String(exitNode) : "");
}

// prev is null until something has been sent.
function identityChanged(prev, cur) {
    return prev === null || prev === undefined || prev !== cur;
}

// Tray icons may only come from the icon provider or an absolute local path.
function safeIconSource(s, fallback) {
    if (typeof s === "string" && (s.indexOf("image://") === 0
        || (s.charAt(0) === "/" && s.charAt(1) !== "/")))
        return s;
    return fallback;
}

// Joins process exit code and collected stdout: output of a failed run is discarded.
function finishRun(code, text) {
    if (code !== 0)
        return { ok: false, text: "" };
    return { ok: true, text: typeof text === "string" ? text : "" };
}

// True when the process never started (binary missing): the only case cached by retryDue.
function failedStart(didStart) {
    return !didStart;
}
