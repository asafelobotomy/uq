.pragma library
.import "notif.js" as Notif

// noti-fi control-socket protocol (spec §2.6; noti-fi docs/protocol.md). Pure: Notifi.qml owns
// the Socket and feeds lines in here.
//
//   server first:  {"event":"hello","name":"notifd","protocol":1,...}
//   request:       {"id":N,"method":"<m>","params":{...}}
//   reply:         {"id":N,"result":...}  or  {"id":N,"error":{"code":"...","message":"..."}}
//   after subscribe: {"event":"state","seq":N,"t":ms,"dnd":bool,...}  (Status fields at top level)
//
// Every line from the daemon is untrusted. uQ caps a line at 64 KiB (UTF-8 bytes) and drops it.

var MAX_LINE = 65536;
var OSD_KINDS = ["volume", "mic", "brightness"];
var MAX_ERROR = 64;

function has(o, k) {
    return Object.prototype.hasOwnProperty.call(o, k);
}

// UTF-8 byte length, stopping once past `limit`. A UTF-16 code unit is at most 3 bytes and a
// surrogate pair (2 units) is 4, so bytes >= units and bytes <= 3 * units.
function overLimit(s, limit) {
    if (s.length > limit)
        return true;
    if (s.length * 3 <= limit)
        return false;
    var n = 0;
    for (var i = 0; i < s.length; i++) {
        var c = s.charCodeAt(i);
        if (c < 0x80) n += 1;
        else if (c < 0x800) n += 2;
        else if (c >= 0xd800 && c <= 0xdbff && i + 1 < s.length) {
            var d = s.charCodeAt(i + 1);
            if (d >= 0xdc00 && d <= 0xdfff) { n += 4; i++; } else n += 3;
        } else n += 3;
        if (n > limit)
            return true;
    }
    return false;
}

function copyPending(p) {
    var out = {};
    if (p && typeof p === "object")
        for (var k in p)
            if (has(p, k)) out[k] = p[k];
    return out;
}

function parseObject(line) {
    var m;
    try { m = JSON.parse(line); } catch (e) { return null; }
    if (m === null || typeof m !== "object" || Array.isArray(m))
        return null;
    return m;
}

function isHello(m) {
    return m !== null && m.event === "hello" && m.name === "notifd" && m.protocol === 1;
}

function errorCode(err) {
    var code = "failed";
    if (typeof err === "string")
        code = err;
    else if (err && typeof err === "object" && typeof err.code === "string")
        code = err.code;
    return code.length > MAX_ERROR ? code.slice(0, MAX_ERROR) : code;
}

// state: {greeted: bool, pending: {id: method}}. Returns {state, effect}; never mutates `state`.
function handleLine(state, line) {
    var greeted = !!(state && state.greeted);
    var pending = copyPending(state && state.pending);
    var next = { greeted: greeted, pending: pending };
    var text = typeof line === "string" ? line : "";
    var big = overLimit(text, MAX_LINE);
    var m = big ? null : parseObject(text);

    if (!greeted) {
        // The first line must be the hello; anything else (oversized included) is not noti-fi.
        if (!isHello(m))
            return { state: next, effect: { kind: "unavailable" } };
        next.greeted = true;
        return { state: next, effect: { kind: "hello" } };
    }
    if (m === null)
        return { state: next, effect: { kind: "ignore" } };

    if (has(m, "event")) {
        if (m.event === "state")
            return { state: next, effect: { kind: "state", status: statusFields(m) } };
        return { state: next, effect: { kind: "ignore" } };
    }

    var id = m.id;
    if ((typeof id !== "number" && typeof id !== "string") || !has(pending, String(id)))
        return { state: next, effect: { kind: "ignore" } };
    var method = pending[String(id)];
    delete pending[String(id)];
    if (m.error !== undefined && m.error !== null)
        return { state: next, effect: { kind: "reply", id: id, method: method, ok: false,
                                        error: errorCode(m.error) } };
    return { state: next, effect: { kind: "reply", id: id, method: method, ok: true,
                                    value: m.result === undefined ? null : m.result } };
}

// One request line, no trailing newline (JSON.stringify escapes any inside strings).
function request(id, method, params) {
    return JSON.stringify({ id: id, method: String(method),
                            params: (params && typeof params === "object") ? params : {} });
}

// Validated `osd` params, or null.
function osdParams(kind, value, max, muted) {
    if (OSD_KINDS.indexOf(kind) < 0)
        return null;
    if (max === undefined)
        max = 100;
    if (typeof max !== "number" || !isFinite(max) || Math.floor(max) !== max || max < 1)
        return null;
    if (typeof value !== "number" || !isFinite(value))
        return null;
    if (muted === undefined)
        muted = false;
    if (typeof muted !== "boolean")
        return null;
    var v = Math.round(value);
    v = v < 0 ? 0 : (v > max ? max : v);
    return { kind: kind, value: v, max: max, muted: muted };
}

var HOLDING = ["", "dnd", "inhibited", "screen sharing", "fullscreen", "focus"];
var MAX_ICON = 512;

// The Status fields uQ uses, validated and capped; null when `o` is not an object. noti-fi's
// `center` is the notification centre being open.
function statusFields(o) {
    if (!o || typeof o !== "object" || Array.isArray(o))
        return null;
    var unread = (typeof o.unread === "number" && isFinite(o.unread) && o.unread >= 0
                  && Math.floor(o.unread) === o.unread) ? o.unread : 0;
    var latest = null;
    var l = o.latest;
    if (l && typeof l === "object" && !Array.isArray(l))
        latest = { appName: Notif.cleanText(l.appName, Notif.MAX_APP),
                   appIcon: Notif.cleanText(l.appIcon, MAX_ICON),
                   summary: Notif.cleanText(l.summary, Notif.MAX_SUMMARY) };
    return { unread: unread, dnd: o.dnd === true, centre: o.center === true,
             sharing: o.sharing === true, latest: latest,
             holding: (typeof o.holding === "string" && HOLDING.indexOf(o.holding) >= 0) ? o.holding : "" };
}

// Status carried by a reply: `subscribe` -> {status, seq}; `dnd` and `center` -> Status. null otherwise.
function replyStatus(method, value) {
    if (!value || typeof value !== "object")
        return null;
    if (method === "subscribe")
        return statusFields(value.status);
    if (method === "dnd" || method === "center")
        return statusFields(value);
    return null;
}

var REQUEST_TIMEOUT_MS = 1000;

// deadlines: {id: deadlineMs}. Ids whose deadline has passed (a non-finite deadline counts as
// passed); a non-finite clock expires nothing.
function expiredIds(deadlines, nowMs) {
    var out = [];
    if (!deadlines || !isFinite(nowMs))
        return out;
    for (var id in deadlines) {
        if (!Object.prototype.hasOwnProperty.call(deadlines, id))
            continue;
        var d = deadlines[id];
        if (!(d > nowMs))
            out.push(id);
    }
    return out;
}
