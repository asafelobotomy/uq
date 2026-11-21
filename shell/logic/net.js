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

// `tailscale status --json` -> {exitNode: hostname|null, exitIp, city, country, selfIp} (the
// last four "" when unknown). The exit node is the peer matching ExitNodeStatus (by ID, then by
// Tailscale IP), or a peer flagged ExitNode. City and country come from the exit node's
// Location, which Tailscale gives for Mullvad exit nodes.
function parseTailscale(json) {
    var none = { exitNode: null, exitIp: "", city: "", country: "", selfIp: "" };
    var o = json;
    if (typeof json === "string") {
        try { o = JSON.parse(json); } catch (e) { return none; }
    }
    if (!o || typeof o !== "object")
        return none;
    var peers = o.Peer && typeof o.Peer === "object" ? o.Peer : {};
    var keys = Object.keys(peers);
    var selfIp = o.Self && Array.isArray(o.Self.TailscaleIPs) && o.Self.TailscaleIPs.length > 0
        ? String(o.Self.TailscaleIPs[0]) : "";
    function found(p) {
        var loc = p.Location && typeof p.Location === "object" ? p.Location : {};
        var pips = Array.isArray(p.TailscaleIPs) ? p.TailscaleIPs : [];
        return {
            exitNode: String(p.HostName),
            exitIp: pips.length > 0 ? String(pips[0]) : "",
            city: loc.City ? String(loc.City) : "",
            country: loc.Country ? String(loc.Country) : "",
            selfIp: selfIp
        };
    }
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
                return found(p);
        }
    }
    for (var k = 0; k < keys.length; k++) {
        var q = peers[keys[k]];
        if (q && q.ExitNode === true && q.HostName)
            return found(q);
    }
    return none;
}

// One nmcli terse field: split at the first unescaped ':' and unescape `\:` and `\\`.
function _terseLine(line) {
    var key = "", val = "", inKey = true;
    for (var j = 0; j < line.length; j++) {
        var c = line[j];
        if (c === "\\" && j + 1 < line.length) {
            if (inKey)
                key += line[j + 1];
            else
                val += line[j + 1];
            j++;
        } else if (c === ":" && inKey) {
            inKey = false;
        } else if (inKey) {
            key += c;
        } else {
            val += c;
        }
    }
    return inKey ? null : { key: key, value: val };
}

// `vpn.data` as nmcli shows it ("remote = a.example:1194, proto-tcp = no") -> {remote: ...}.
function parseVpnData(text) {
    var out = {};
    var parts = String(text || "").split(/,\s*/);
    for (var i = 0; i < parts.length; i++) {
        var eq = parts[i].indexOf("=");
        if (eq <= 0)
            continue;
        var k = parts[i].substring(0, eq).trim(), v = parts[i].substring(eq + 1).trim();
        if (k !== "" && !(k in out))
            out[k] = v;
    }
    return out;
}

var _VPN_KINDS = {
    "openvpn": "OpenVPN",
    "openconnect": "OpenConnect",
    "l2tp": "L2TP",
    "pptp": "PPTP",
    "vpnc": "Cisco IPsec",
    "strongswan": "IPsec (IKEv2)",
    "libreswan": "IPsec",
    "fortisslvpn": "Fortinet SSL",
    "sstp": "SSTP",
    "ssh": "SSH"
};

// `nmcli -t connection show id <name>` for an active VPN -> {kind, server, device, address,
// gateway}, each "" when nmcli doesn't say. kind is the protocol people know it by
// ("WireGuard", "OpenVPN over UDP"); server is where the tunnel goes (OpenVPN's remote, a
// plugin's gateway), never secrets: vpn.secrets is not read, and nmcli hides them without -s.
function parseNmcliConnection(text) {
    var out = { kind: "", server: "", device: "", address: "", gateway: "" };
    if (typeof text !== "string")
        return out;
    var f = {};
    var lines = text.split("\n");
    for (var i = 0; i < lines.length; i++) {
        var kv = _terseLine(lines[i]);
        if (kv && !(kv.key in f))
            f[kv.key] = kv.value;
    }
    var type = f["connection.type"] || "";
    if (type === "wireguard") {
        out.kind = "WireGuard";
    } else if (type === "vpn") {
        var service = String(f["vpn.service-type"] || "");
        var short = service.substring(service.lastIndexOf(".") + 1);
        var data = parseVpnData(f["vpn.data"]);
        out.kind = _VPN_KINDS[short] || (short !== "" ? short : "VPN");
        if (short === "openvpn")
            out.kind += data["proto-tcp"] === "yes" ? " over TCP" : " over UDP";
        if (short === "openconnect" && data.protocol)
            out.kind += " (" + data.protocol + ")";
        var server = data.remote || data.gateway || data["IPSec gateway"] || data.address || "";
        out.server = String(server).split(/[\s,]+/)[0];
    } else if (type !== "") {
        out.kind = type;
    }
    out.device = String(f["GENERAL.DEVICES"] || f["connection.interface-name"] || "");
    if (out.device === "--")
        out.device = "";
    out.address = String(f["IP4.ADDRESS[1]"] || f["IP6.ADDRESS[1]"] || "");
    out.gateway = String(f["IP4.GATEWAY"] || "");
    if (out.gateway === "--")
        out.gateway = "";
    return out;
}

// `uqbit status --json` -> {required, interface, up, bound} (its torrent kill switch), or null.
function parseUqbit(text) {
    var o;
    try { o = JSON.parse(text); } catch (e) { return null; }
    var v = o && typeof o === "object" ? o.vpn : null;
    if (!v || typeof v !== "object")
        return null;
    return { required: v.required === true, interface: typeof v.interface === "string" ? v.interface : "",
        up: v.up === true, bound: v.bound === true };
}

// The VPN dropdown's rows: [{label, value}] from VpnStatus's name, nmcli details, Tailscale
// details and the bridge's PublicIp (null while the lookup is off, failed or not done). With
// `torrent` is parseUqbit()'s result (a "Torrents" row while uQbit requires the VPN). With
// `sharing` the server, public address and place are left out, as the bar leaves out the country.
// Values are untrusted and only reach UqText.
function vpnRows(name, nm, ts, ip, sharing, torrent) {
    var rows = [];
    function add(label, value) {
        if (value !== undefined && value !== null && String(value) !== "")
            rows.push({ label: label, value: String(value) });
    }
    add("Name", name);
    if (nm && nm.kind) {
        add("Type", nm.kind);
        if (!sharing)
            add("Server", nm.server);
        add("Interface", nm.device);
        add("VPN address", nm.address);
    } else if (ts && ts.exitNode) {
        add("Type", "Tailscale exit node (WireGuard)");
        add("Exit node", ts.exitNode + (ts.exitIp ? " · " + ts.exitIp : ""));
        add("Tailnet address", ts.selfIp);
    }
    if (torrent && torrent.required) {
        if (torrent.up && torrent.bound)
            add("Torrents", "only through " + (torrent.interface || "the VPN"));
        else
            add("Torrents", "stopped, the VPN is down");
    }
    if (!sharing) {
        var place = [];
        if (ip && ip.city)
            place.push(ip.city);
        if (ip && ip.region && ip.region !== ip.city)
            place.push(ip.region);
        if (ip && ip.country_code)
            place.push(ip.country_code);
        if (place.length === 0 && ts && ts.exitNode) {
            if (ts.city)
                place.push(ts.city);
            if (ts.country)
                place.push(ts.country);
        }
        add("Location", place.join(", "));
        add("Public address", ip ? ip.ip : "");
        add("Provider", ip ? ip.org : "");
    }
    return rows;
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
