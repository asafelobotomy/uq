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
