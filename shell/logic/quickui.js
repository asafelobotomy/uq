.pragma library
.import "quick.js" as Q

// Pure helpers for the quick-settings window's controls (QuickSettings and its groups).
// No Quickshell imports; live Pipewire/UPower objects are only read through the plain fields
// named below, so tests can pass plain objects.

// Fixed argv for an absolute brightness. The percent is rounded and clamped to 0..100
// (brightnessctl's -n2 keeps the panel lit at 0). Anything that isn't a finite number -> null.
function brightnessSetArgs(percent) {
    if (typeof percent !== "number" || !isFinite(percent))
        return null;
    var n = Math.max(0, Math.min(100, Math.round(percent)));
    return ["brightnessctl", "-m", "-e4", "-n2", "set", n + "%"];
}

// A slider move from `prev` to `next` (both 0..1). Raising from 0 while muted unmutes; any
// other move leaves mute as it is. A non-finite `next` -> null (no change).
function volumeSet(prev, next, muted) {
    var n = Number(next);
    if (next === null || next === undefined || !isFinite(n))
        return null;
    n = Math.max(0, Math.min(1, n));
    var p = Number(prev);
    var fromZero = !(p > 0);
    var unmute = !!muted && fromZero && n > 0;
    // Skip writes that don't change the whole percent: a drag sends a move per frame.
    if (!unmute && isFinite(p) && Math.round(n * 100) === Math.round(p * 100))
        return null;
    return { volume: n, unmute: unmute };
}

// Hardware audio sinks from Pipewire.nodes.values: isSink, not a stream, and with audio.
function sinkNodes(nodes) {
    var out = [];
    if (!nodes)
        return out;
    for (var i = 0; i < nodes.length; i++) {
        var n = nodes[i];
        if (n && n.isSink && !n.isStream && n.audio)
            out.push(n);
    }
    return out;
}

// A node's label: description, then nickname, then name. Untrusted text, returned unchanged
// for UqText (plain text, elided).
function nodeLabel(node) {
    if (!node)
        return "Unknown device";
    var keys = ["description", "nickname", "name"];
    for (var i = 0; i < keys.length; i++) {
        var v = node[keys[i]];
        if (v !== null && v !== undefined && String(v) !== "")
            return String(v);
    }
    return "Unknown device";
}

// PowerProfile values offered by the selector (PowerSaver=0, Balanced=1, Performance=2).
function profileOptions(hasPerformance) {
    return hasPerformance ? [0, 1, 2] : [0, 1];
}

// The battery row shows only for a present laptop battery.
function batteryShown(device) {
    return !!device && !!device.isLaptopBattery && !!device.isPresent;
}

// --- network ---

// WifiNetwork-shaped objects ({name, signalStrength, connected, known, security}) -> quick.js
// wifiRows rows, each with `index` into `networks` so a tap reaches the live object. Only
// `openSecurity` (WifiSecurityType.Open) counts as unsecured; Unknown and the rest are secured,
// so they are never connectable unless known. Names are untrusted and passed through untouched.
function wifiList(networks, openSecurity) {
    if (!networks)
        return [];
    var inputs = [];
    for (var i = 0; i < networks.length; i++) {
        var n = networks[i];
        if (!n)
            continue;
        inputs.push({
            name: n.name,
            signal: n.signalStrength,
            connected: !!n.connected,
            known: !!n.known,
            secure: n.security !== openSecurity,
            index: i
        });
    }
    var rows = Q.wifiRows(inputs);
    var used = {};
    function same(a, r) {
        return a.name === r.name && a.connected === r.connected && a.known === r.known
            && a.secure === r.secure;
    }
    function pct(v) {
        var x = Number(v);
        return isFinite(x) ? Math.round(Math.max(0, Math.min(1, x)) * 100) : 0;
    }
    rows.forEach(function (r) {
        var pick = -1;
        for (var j = 0; j < inputs.length && pick < 0; j++)
            if (!used[j] && same(inputs[j], r) && pct(inputs[j].signal) === r.signal)
                pick = j;
        for (var k = 0; k < inputs.length && pick < 0; k++)
            if (!used[k] && same(inputs[k], r))
                pick = k;
        used[pick] = true;
        r.index = pick < 0 ? -1 : inputs[pick].index;
    });
    return rows;
}

// What a tap on a Wi-Fi row does: "disconnect", "connect", or "setup" for a secured network
// that isn't known (the shell has no password prompt; it only says "Set up in Settings").
function wifiAction(row) {
    if (!row)
        return "";
    if (row.connected)
        return "disconnect";
    return row.connectable ? "connect" : "setup";
}

// A Wi-Fi row's status text: "Open" for an unsecured network (with " · Connected" when it is),
// "Connected", "Set up in Settings" for a secured one that can't be joined here, or "".
function wifiStatus(row) {
    if (!row)
        return "";
    var parts = [];
    if (!row.secure)
        parts.push("Open");
    var a = wifiAction(row);
    if (a === "disconnect")
        parts.push("Connected");
    else if (a === "setup")
        parts.push("Set up in Settings");
    return parts.join(" · ");
}

// One line for a WiredDevice ({connected, hasLink, linkSpeed in Mb/s}).
function wiredLabel(dev) {
    if (!dev)
        return "";
    if (dev.connected) {
        var s = Number(dev.linkSpeed);
        return "Wired · Connected" + (s > 0 ? " · " + Math.round(s) + " Mb/s" : "");
    }
    return dev.hasLink ? "Wired · Cable plugged in, not connected" : "Wired · Cable unplugged";
}

// --- Bluetooth ---

// Paired devices, connected first, otherwise in the order given.
function btDevices(devices) {
    var on = [], off = [];
    if (!devices)
        return on;
    for (var i = 0; i < devices.length; i++) {
        var d = devices[i];
        if (d && d.paired)
            (d.connected ? on : off).push(d);
    }
    return on.concat(off);
}

// A device's label: name, then deviceName, then address. Untrusted text, returned unchanged
// for UqText (plain text, elided).
function btLabel(dev) {
    if (!dev)
        return "Unknown device";
    var keys = ["name", "deviceName", "address"];
    for (var i = 0; i < keys.length; i++) {
        var v = dev[keys[i]];
        if (v !== null && v !== undefined && String(v) !== "")
            return String(v);
    }
    return "Unknown device";
}

// "80%" when the device reports a battery (0..1), else "".
function btBattery(dev) {
    if (!dev || !dev.batteryAvailable)
        return "";
    var b = Number(dev.battery);
    if (!isFinite(b))
        return "";
    return Math.round(Math.max(0, Math.min(1, b)) * 100) + "%";
}

// --- theme ---

// GetTheme's reply [name, variant] -> {name, variant}, or null if malformed.
function themeReply(v) {
    if (!Array.isArray(v) || v.length !== 2)
        return null;
    var name = v[0], variant = v[1];
    if (typeof name !== "string" || name === "")
        return null;
    if (variant !== "dark" && variant !== "light")
        return null;
    return { name: name, variant: variant };
}

function otherVariant(variant) {
    return variant === "dark" ? "light" : "dark";
}

// ListThemes' reply [ids] -> the non-empty string ids.
function themeList(v) {
    if (!Array.isArray(v))
        return [];
    return v.filter(function (id) { return typeof id === "string" && id !== ""; });
}

// ListThemeInfo's reply {themes: [{id, name, ...}]} -> [{id, name}] for the quick-settings chips.
// Only string ids pass; a name that is missing or not a string becomes the id; names are cut to
// 24 characters.
function themeInfoList(v) {
    if (!v || typeof v !== "object" || !Array.isArray(v.themes))
        return [];
    var out = [];
    v.themes.forEach(function (t) {
        if (!t || typeof t.id !== "string" || t.id === "")
            return;
        var name = typeof t.name === "string" && t.name !== "" ? t.name : t.id;
        out.push({ id: t.id, name: name.length > 24 ? name.substring(0, 24) : name });
    });
    return out;
}

// The workspace selector uqd stores layouts under, exactly as uqctl's active_workspace_selector
// (crates/uqctl/src/main.rs) builds it: the id when > 0 (named and special workspaces have
// negative ids), a special workspace's own name, otherwise "name:<name>". "" when unknown.
function workspaceSelector(ws) {
    if (!ws)
        return "";
    var id = Number(ws.id);
    if (isFinite(id) && id > 0 && Math.floor(id) === id)
        return String(id);
    var name = typeof ws.name === "string" ? ws.name : "";
    if (name === "")
        return "";
    return name.indexOf("special") === 0 ? name : "name:" + name;
}

// The default layout uqd renders (settings' layout_options.default), "" when unset.
function defaultLayout(settings) {
    var o = settings && typeof settings === "object" ? settings.layout_options : null;
    var d = o && typeof o === "object" ? o["default"] : null;
    return typeof d === "string" ? d : "";
}

// GetLayouts' map ("<id>" or "name:<name>" -> layout) for a workspace {id, name}. A workspace
// without a rule has `fallback`, the default layout (defaultLayout(settings)), or dwindle
// (Hyprland's default) when that is unset. No workspace -> "".
function workspaceLayout(layouts, ws, fallback) {
    if (!ws)
        return "";
    var m = layouts && typeof layouts === "object" ? layouts : {};
    var sel = workspaceSelector(ws);
    var dflt = typeof fallback === "string" && fallback !== "" ? fallback : "dwindle";
    return (sel !== "" && m[sel]) || m["name:" + ws.name] || dflt;
}

// --- profiles ---

// ListProfiles' reply -> [{id, name, active}], at most 50 rows. A row needs a non-empty string
// id and name; `active` is true only for a literal true. Names are untrusted text for UqText.
// Anything that isn't an array -> [].
function profileList(v) {
    if (!Array.isArray(v))
        return [];
    var out = [];
    for (var i = 0; i < v.length && out.length < 50; i++) {
        var p = v[i];
        if (!p || typeof p !== "object")
            continue;
        if (typeof p.id !== "string" || p.id === "" || typeof p.name !== "string" || p.name === "")
            continue;
        out.push({ id: p.id, name: p.name, active: p.active === true });
    }
    return out;
}

// The active row's id from profileList's rows, or "" when none is on.
function activeProfile(list) {
    if (!Array.isArray(list))
        return "";
    for (var i = 0; i < list.length; i++)
        if (list[i] && list[i].active === true && typeof list[i].id === "string")
            return list[i].id;
    return "";
}

// One line from an ApplyProfile / ProfileOff report {active, kept: [{label}], notices}:
// "Kept your own value: A, B" when Back to normal kept hand-changed keys, then each notice,
// joined with a space. A malformed report -> "".
function profileReportText(r) {
    if (!r || typeof r !== "object" || !Array.isArray(r.kept) || !Array.isArray(r.notices))
        return "";
    var parts = [];
    var labels = r.kept.filter(function (k) {
        return k && typeof k.label === "string" && k.label !== "";
    }).map(function (k) { return k.label; });
    if (labels.length > 0)
        parts.push("Kept your own value: " + labels.join(", "));
    r.notices.forEach(function (n) {
        if (typeof n === "string" && n !== "")
            parts.push(n);
    });
    return parts.join(" ");
}

// Whether two profileList results show the same chips (id, name and active, in order), so the
// row keeps its chips, and keyboard focus, when a refetch brings nothing new.
function sameProfiles(a, b) {
    if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length)
        return false;
    for (var i = 0; i < a.length; i++) {
        var x = a[i], y = b[i];
        if (!x || !y || x.id !== y.id || x.name !== y.name || x.active !== y.active)
            return false;
    }
    return true;
}

// The line a failed ApplyProfile / ProfileOff shows. uqd's own refusals are plain words and
// shown as they are; the bridge's transport errors (they name uqd or the bridge) become one
// plain line, and anything else that isn't a usable string becomes a generic one.
function profileErrorText(e) {
    if (typeof e !== "string" || e.trim() === "")
        return "Couldn't switch profiles.";
    if (/uqd|bridge|d-?bus/i.test(e) || e === "reset")
        return "Couldn't reach uQ's settings service. Is it running?";
    if (/^(bad profile|unknown call)/.test(e))
        return "Couldn't switch profiles.";
    return e;
}
