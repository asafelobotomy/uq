.pragma library

// Notification backend choice, the Notifications island's view, and parsers for swaync, mako
// and dunst output (spec §2.5). Pure: the QML backends own the processes and feed text in here.
// Everything read from a daemon is untrusted; every string that reaches a view goes through
// cleanText.

var NOTIFI_GRACE_MS = 5000;
var MAX_APP = 64;
var MAX_SUMMARY = 200;

var EXTERNAL = ["swaync", "mako", "dunst"];

// Control characters (C0, DEL, C1), line/paragraph separators and bidi overrides become spaces;
// trimmed; capped at `max` with an ellipsis.
function cleanText(s, max) {
    if (typeof s !== "string")
        return "";
    var t = s.replace(/[\u0000-\u001f\u007f-\u009f\u2028\u2029\u202A-\u202E\u2066-\u2069]/g, " ").trim();
    if (t.length > max)
        t = t.slice(0, Math.max(0, max - 1)) + "…";
    return t;
}

function fallback(env, warning) {
    if (!env.graceOver)
        return { backend: "pending", warning: "" };
    return { backend: "builtin", warning: warning };
}

function chooseBackend(setting, env) {
    if (setting === "builtin")
        return { backend: "builtin", warning: "" };
    if (EXTERNAL.indexOf(setting) >= 0) {
        var d = env.detected ? env.detected[setting] : null;
        if (d === true)
            return { backend: setting, warning: "" };
        if (d === false)
            return { backend: "builtin", warning: setting + " is not installed" };
        return { backend: "pending", warning: "" };
    }
    if (env.notifi)
        return { backend: "notifi", warning: "" };
    return fallback(env, setting === "notifi" ? "noti-fi is not running" : "");
}

function builtinShouldRegister(backend, owner) {
    return backend === "builtin" && !!owner && owner.known === true
        && (owner.owner === null || owner.self === true);
}

function countLabel(n) {
    if (typeof n !== "number" || !isFinite(n) || n <= 0)
        return "";
    if (n > 99)
        return "99+";
    return String(Math.floor(n));
}

function islandView(b, opts) {
    var o = opts || {};
    var sharing = !!o.sharing || !!(b && b.sharing);
    var latest = b && b.latest ? b.latest : null;
    var summary = "";
    if (latest && (o.hovered || o.showSummary) && !sharing)
        summary = cleanText(latest.summary, MAX_SUMMARY);
    var name = cleanText(o.yieldedTo, MAX_APP);
    return {
        count: b && typeof b.count === "number" ? b.count : 0,
        appIcon: latest && typeof latest.appIcon === "string" ? latest.appIcon : "",
        summary: summary,
        yieldedTo: o.yieldedTo === undefined || o.yieldedTo === null || o.yieldedTo === ""
            ? "" : "Notifications handled by " + (name || "another app"),
        dnd: !!(b && b.dnd)
    };
}

function parseSwaync(line) {
    if (typeof line !== "string")
        return null;
    var o;
    try {
        o = JSON.parse(line);
    } catch (e) {
        return null;
    }
    if (!o || typeof o !== "object" || Array.isArray(o))
        return null;
    if (typeof o.count === "number" && isFinite(o.count))
        return { count: Math.max(0, Math.floor(o.count)), dnd: o.dnd === true, centre: false };
    if (typeof o.text === "string" && typeof o.alt === "string") {
        var txt = o.text.trim();
        if (txt === "" || /^\d+$/.test(txt))
            return { count: txt === "" ? 0 : parseInt(txt, 10), dnd: o.alt.indexOf("dnd") === 0, centre: false };
    }
    return null;
}

function makoEntry(app, summary) {
    return { appIcon: "", appName: cleanText(app, MAX_APP), summary: cleanText(summary, MAX_SUMMARY) };
}

function jsonField(e, k) {
    var f = e ? e[k] : null;
    return f && typeof f === "object" ? f.data : f;
}

function parseMakoList(out) {
    var empty = { count: 0, latest: null };
    if (typeof out !== "string")
        return empty;
    var t = out.trim();
    if (t.charAt(0) === "{") {
        var o;
        try {
            o = JSON.parse(t);
        } catch (e) {
            return empty;
        }
        var list = o && Array.isArray(o.data) && Array.isArray(o.data[0]) ? o.data[0] : [];
        if (list.length === 0)
            return empty;
        return {
            count: list.length,
            latest: makoEntry(jsonField(list[0], "app-name"), jsonField(list[0], "summary"))
        };
    }
    var lines = out.split("\n");
    var count = 0;
    var entries = [];
    for (var i = 0; i < lines.length; i++) {
        var m = /^Notification \d+: ?(.*)$/.exec(lines[i]);
        if (m) {
            count++;
            if (count === 1)
                entries.push({ summary: m[1], app: "" });
            continue;
        }
        var a = /^\s+App name: ?(.*)$/.exec(lines[i]);
        if (a && count === 1 && entries.length === 1 && entries[0].app === "")
            entries[0].app = a[1];
    }
    if (count === 0)
        return empty;
    return { count: count, latest: makoEntry(entries[0].app, entries[0].summary) };
}

function parseMakoModes(out) {
    if (typeof out !== "string")
        return false;
    var lines = out.split("\n");
    for (var i = 0; i < lines.length; i++)
        if (lines[i].trim() === "do-not-disturb")
            return true;
    return false;
}

function parseDunstCount(out) {
    if (typeof out !== "string")
        return 0;
    var n = 0;
    var keys = [/^\s*Waiting:\s*(\d+)\s*$/m, /^\s*Currently displayed:\s*(\d+)\s*$/m];
    for (var i = 0; i < keys.length; i++) {
        var m = keys[i].exec(out);
        if (m)
            n += parseInt(m[1], 10);
    }
    return isFinite(n) ? n : 0;
}

function parseDunstPaused(out) {
    return typeof out === "string" && out.trim() === "true";
}

// --- change events (gdbus monitor) ---
// Which of a daemon's states arrive as D-Bus change signals, so its adapter can stop polling them:
// { dnd, count }, both false when unknown. Everything here parses untrusted `gdbus` output.

var MAX_MONITOR_LINE = 4096;
var NO_EVENTS = { dnd: false, count: false };

// `[major, minor, patch]` from a version string such as "1.13.2 (2026-03-18)" or
// "v1.13.1-12-gabc123" (git describe); null when there is none.
function parseVersion(s) {
    if (typeof s !== "string")
        return null;
    var m = /^\s*v?(\d{1,4})\.(\d{1,4})(?:\.(\d{1,4}))?/.exec(s);
    if (!m)
        return null;
    return [parseInt(m[1], 10), parseInt(m[2], 10), m[3] === undefined ? 0 : parseInt(m[3], 10)];
}

function versionAtLeast(v, min) {
    if (!v)
        return false;
    for (var i = 0; i < 3; i++) {
        if (v[i] !== min[i])
            return v[i] > min[i];
    }
    return true;
}

// `gdbus call ... org.freedesktop.Notifications.GetServerInformation` prints
// ('name', 'vendor', 'version', 'spec'); {name, vendor, version} or null.
function parseServerInfo(out) {
    if (typeof out !== "string" || out.length > MAX_MONITOR_LINE)
        return null;
    var m = /^\('([^'\\]*)', '([^'\\]*)', '([^'\\]*)', '([^'\\]*)'\)\s*$/.exec(out.trim());
    if (!m)
        return null;
    return { name: m[1], vendor: m[2], version: m[3] };
}

// dunst emits PropertiesChanged on org.dunstproject.cmd0 for displayedLength / historyLength /
// waitingLength since 1.10.0 (queues_update) and for paused / pauseLevel since 1.13.1 (set
// through D-Bus or SIGUSR1/2). Older builds change silently, so they stay polled.
function dunstEvents(serverInfo) {
    var info = parseServerInfo(serverInfo);
    if (!info || info.name !== "dunst")
        return NO_EVENTS;
    var v = parseVersion(info.version);
    return { dnd: versionAtLeast(v, [1, 13, 1]), count: versionAtLeast(v, [1, 10, 0]) };
}

// mako reports version 0.0.0, so it is probed by introspecting /fr/emersion/Mako: the Modes
// property (mako 1.10.0) and the Notifications property (1.11.0) each came with a
// PropertiesChanged (invalidation) on every change.
function makoEvents(introspection) {
    if (typeof introspection !== "string" || introspection.length > 65536)
        return NO_EVENTS;
    var m = /<interface name="fr\.emersion\.Mako">([\s\S]*?)<\/interface>/.exec(introspection);
    if (!m)
        return NO_EVENTS;
    return {
        dnd: /<property name="Modes" type="as"/.test(m[1]),
        count: /<property name="Notifications" type="aa\{sv\}"/.test(m[1])
    };
}

// One `gdbus monitor --dest <name> --object-path <path>` line: "owner" when the daemon (re)took
// the name (the subscription is in place), "lost" when it dropped it, "change" for a
// PropertiesChanged of `iface` on `path`, else null. gdbus only shows signals from the name's
// current owner, so other clients cannot fake these.
function monitorLine(line, name, path, iface) {
    if (typeof line !== "string" || line.length > MAX_MONITOR_LINE)
        return null;
    if (line === "The name " + name + " does not have an owner")
        return "lost";
    if (line.indexOf("The name " + name + " is owned by ") === 0)
        return "owner";
    if (line.indexOf(path + ": org.freedesktop.DBus.Properties.PropertiesChanged ('" + iface + "',") === 0)
        return "change";
    return null;
}

// What the 5 s poll still has to read: the parts that no live monitor reports.
function pollNeeds(events, live) {
    var e = events || NO_EVENTS;
    return { dnd: !(live && e.dnd === true), count: !(live && e.count === true) };
}
