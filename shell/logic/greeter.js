.pragma library

// uq-greeter's decisions (spec phase 5 §2): its config and state files, the session list, the
// user list, the greetd conversation, power confirmation and the "session ended straight away"
// check. Pure: greeter/shell.qml owns the files, processes and the Greetd object and feeds their
// output in here, so qmltestrunner can test all of it with fakes.
//
// Nothing here logs. Passwords are only ever returned as a `respond` action.

.import "auth.js" as Auth
.import "notif.js" as Notif

// --- fixed paths the greeter may read (§2.2) ---
var CONFIG_PATH = "/etc/uq/greeter.toml";
var RENDERED_DIR = "/usr/share/uq/themes/rendered";
var WALLPAPER_PATH = "/var/lib/uq-greeter/wallpaper";
var SESSIONS_DIR = "/usr/share/wayland-sessions";
var STATE_PATH = "/var/cache/uq-greeter/state.toml";
var ICONS_DIR = "/var/lib/AccountsService/icons/";

var DEFAULT_THEME = "graphite";
var DEFAULT_VARIANT = "dark";
var UQ_SESSION_ID = "uq.desktop";
var UQ_SESSION_EXEC = "/usr/bin/uq-session";

// A launch followed by the greeter coming back within this long means the session failed.
var ENDED_EARLY_MS = 15000;
// A power button's first click arms it for this long; a second click in that time runs it.
var POWER_CONFIRM_MS = 5000;
var MAX_USERS = 200;
var MAX_SESSIONS = 50;

// --- a small TOML reader for greeter.toml and state.toml ---
// Only what those files use: comments, [table] and ["quoted"] headers, bare or "quoted" keys,
// and string, integer and boolean values. Anything else on a line makes that line be skipped.
function _unescape(body) {
    var out = "";
    for (var i = 0; i < body.length; i++) {
        var c = body[i];
        if (c !== "\\") {
            out += c;
            continue;
        }
        var n = body[++i];
        if (n === "\"" || n === "\\") out += n;
        else if (n === "n") out += "\n";
        else if (n === "t") out += "\t";
        else if (n === "r") out += "\r";
        else if (n === "u" && /^[0-9a-fA-F]{4}$/.test(body.substr(i + 1, 4))) {
            out += String.fromCharCode(parseInt(body.substr(i + 1, 4), 16));
            i += 4;
        } else return null;
    }
    return out;
}

// A basic string at the start of s: {value, rest}, or null.
function _string(s) {
    var m = /^"((?:[^"\\\u0000-\u001f]|\\.)*)"/.exec(s);
    if (!m)
        return null;
    var v = _unescape(m[1]);
    return v === null ? null : { value: v, rest: s.slice(m[0].length) };
}

function _key(s) {
    var m = /^[A-Za-z0-9_-]+/.exec(s);
    if (m)
        return { value: m[0], rest: s.slice(m[0].length) };
    return _string(s);
}

function _trailing(rest) {
    return /^\s*(#.*)?$/.test(rest);
}

// {"": {top-level keys}, "<table>": {...}}.
function parseToml(text) {
    var doc = { "": {} };
    var table = doc[""];
    if (typeof text !== "string")
        return doc;
    var lines = text.split(/\r?\n/);
    for (var i = 0; i < lines.length; i++) {
        var line = lines[i].replace(/^\s+/, "");
        if (line === "" || line[0] === "#")
            continue;
        if (line[0] === "[") {
            var k = _key(line.slice(1).replace(/^\s+/, ""));
            if (!k || !/^\s*\]/.test(k.rest) || !_trailing(k.rest.replace(/^\s*\]/, ""))) {
                table = {};  // a header we can't read: its keys go nowhere
                continue;
            }
            if (!Object.prototype.hasOwnProperty.call(doc, k.value))
                doc[k.value] = {};
            table = doc[k.value];
            continue;
        }
        var key = _key(line);
        if (!key)
            continue;
        var eq = /^\s*=\s*/.exec(key.rest);
        if (!eq)
            continue;
        var v = key.rest.slice(eq[0].length);
        var value;
        var s = _string(v);
        if (s) {
            if (!_trailing(s.rest)) continue;
            value = s.value;
        } else {
            var m = /^(true|false|[+-]?[0-9]+)(?=\s|#|$)/.exec(v);
            if (!m || !_trailing(v.slice(m[0].length))) continue;
            value = m[1] === "true" ? true : m[1] === "false" ? false : parseInt(m[1], 10);
        }
        table[key.value] = value;
    }
    return doc;
}

function tomlString(s) {
    var out = "\"";
    var str = String(s);
    for (var i = 0; i < str.length; i++) {
        var c = str[i];
        var code = str.charCodeAt(i);
        if (c === "\"" || c === "\\") out += "\\" + c;
        else if (code < 0x20 || code === 0x7f) out += "\\u" + ("000" + code.toString(16)).slice(-4);
        else out += c;
    }
    return out + "\"";
}

function _bool(v, d) {
    return typeof v === "boolean" ? v : d;
}

// --- /etc/uq/greeter.toml ---
var NAME_RE = /^[a-z0-9][a-z0-9_-]{0,63}$/;

// {theme, variant, hideUsers, reduceMotion}; bad or missing values fall back to the defaults.
function greeterConfig(text) {
    var t = parseToml(text)[""];
    var theme = typeof t.theme === "string" && NAME_RE.test(t.theme) ? t.theme : DEFAULT_THEME;
    var variant = t.variant === "light" || t.variant === "dark" ? t.variant : DEFAULT_VARIANT;
    return { theme: theme, variant: variant, hideUsers: _bool(t.hide_users, false),
             reduceMotion: _bool(t.reduce_motion, false) };
}

// The pre-rendered tokens.json for a config; the default theme's when `fallback` is set.
function tokensPath(config, fallback) {
    if (fallback || !config)
        return RENDERED_DIR + "/" + DEFAULT_THEME + "-" + DEFAULT_VARIANT + "/tokens.json";
    return RENDERED_DIR + "/" + config.theme + "-" + config.variant + "/tokens.json";
}

// --- /var/cache/uq-greeter/state.toml ---
// {lastUser, sessions: {user: session id}, launchedAt (ms, 0 = none), launchedUser}.
function parseState(text) {
    var doc = parseToml(text);
    var top = doc[""];
    var st = { lastUser: "", sessions: {}, launchedAt: 0, launchedUser: "" };
    if (typeof top.last_user === "string" && validUserName(top.last_user))
        st.lastUser = top.last_user;
    if (typeof top.launched_at === "number" && isFinite(top.launched_at) && top.launched_at > 0)
        st.launchedAt = top.launched_at;
    if (typeof top.launched_user === "string" && validUserName(top.launched_user))
        st.launchedUser = top.launched_user;
    var sessions = doc.sessions || {};
    for (var u in sessions) {
        if (validUserName(u) && typeof sessions[u] === "string" && validSessionId(sessions[u]))
            st.sessions[u] = sessions[u];
    }
    return st;
}

function serializeState(st) {
    var out = "# Written by uq-greeter: the last user and each user's last session.\n";
    if (st.lastUser)
        out += "last_user = " + tomlString(st.lastUser) + "\n";
    if (st.launchedAt > 0) {
        out += "launched_at = " + Math.round(st.launchedAt) + "\n";
        out += "launched_user = " + tomlString(st.launchedUser || "") + "\n";
    }
    var users = Object.keys(st.sessions || {}).sort();
    if (users.length > 0) {
        out += "\n[sessions]\n";
        for (var i = 0; i < users.length; i++)
            out += tomlString(users[i]) + " = " + tomlString(st.sessions[users[i]]) + "\n";
    }
    return out;
}

// The state to write just before launching `sessionId` for `user`.
function recordLaunch(st, user, sessionId, nowMs) {
    var sessions = {};
    for (var u in (st && st.sessions) || {})
        sessions[u] = st.sessions[u];
    sessions[user] = sessionId;
    return { lastUser: user, sessions: sessions, launchedAt: nowMs, launchedUser: user };
}

// The greeter came back within ENDED_EARLY_MS of launching a session: that session ended
// straight away. Returns the user to keep selected, or "".
function endedEarly(st, nowMs) {
    if (!st || !st.launchedAt || !st.launchedUser)
        return "";
    var age = nowMs - st.launchedAt;
    return age >= 0 && age <= ENDED_EARLY_MS ? st.launchedUser : "";
}

// The state once the "ended straight away" check is done (so it shows once).
function clearLaunch(st) {
    return { lastUser: st.lastUser, sessions: st.sessions, launchedAt: 0, launchedUser: "" };
}

var ENDED_EARLY_TEXT = "The session ended straight away";

// --- sessions (/usr/share/wayland-sessions/*.desktop) ---
function validSessionId(id) {
    return typeof id === "string" && /^[A-Za-z0-9._+-]{1,120}\.desktop$/.test(id) && id[0] !== ".";
}

// The string-type unescaping of a desktop-entry value: \s \n \t \r \\ (applied before the Exec
// quoting rules).
function _desktopString(v) {
    return v.replace(/\\([sntr\\])/g, function (m, c) {
        return c === "s" ? " " : c === "n" ? "\n" : c === "t" ? "\t" : c === "r" ? "\r" : "\\";
    });
}

// Name and Exec from the [Desktop Entry] group (unlocalised keys only); null without both.
function parseDesktopEntry(text) {
    if (typeof text !== "string")
        return null;
    var lines = text.split(/\r?\n/);
    var inGroup = false;
    var name = null, exec = null;
    for (var i = 0; i < lines.length; i++) {
        var line = lines[i];
        if (/^\s*(#|$)/.test(line))
            continue;
        var g = /^\[(.*)\]\s*$/.exec(line);
        if (g) {
            inGroup = g[1] === "Desktop Entry";
            continue;
        }
        if (!inGroup)
            continue;
        var kv = /^([A-Za-z0-9-]+)\s*=\s*(.*)$/.exec(line);
        if (!kv)
            continue;
        if (kv[1] === "Name" && name === null)
            name = _desktopString(kv[2]);
        else if (kv[1] === "Exec" && exec === null)
            exec = _desktopString(kv[2]);
    }
    if (name === null || exec === null || exec.trim() === "")
        return null;
    return { name: Notif.cleanText(name, 80), exec: exec };
}

// Split an Exec value (already string-unescaped) into argv by the desktop-entry rules: arguments
// are separated by spaces; a double-quoted argument may contain spaces, and inside it \" \` \$
// and \\ stand for the character. Field codes (%f %U …) are dropped, %% is a literal %. Returns
// null for an unterminated quote or an empty command.
function splitExec(exec) {
    if (typeof exec !== "string")
        return null;
    var args = [];
    var cur = "";
    var have = false;
    var i = 0;
    while (i < exec.length) {
        var c = exec[i];
        if (c === " " || c === "\t") {
            if (have) { args.push(cur); cur = ""; have = false; }
            i++;
            continue;
        }
        if (c === "\"") {
            have = true;
            i++;
            var closed = false;
            while (i < exec.length) {
                var q = exec[i];
                if (q === "\\" && i + 1 < exec.length && "\"`$\\".indexOf(exec[i + 1]) >= 0) {
                    cur += exec[i + 1];
                    i += 2;
                    continue;
                }
                if (q === "\"") { closed = true; i++; break; }
                cur += q;
                i++;
            }
            if (!closed)
                return null;
            continue;
        }
        if (c === "%" && i + 1 < exec.length) {
            var f = exec[i + 1];
            if (f === "%") cur += "%";
            // any other field code expands to nothing in a session command
            have = have || f === "%";
            i += 2;
            continue;
        }
        cur += c;
        have = true;
        i++;
    }
    if (have)
        args.push(cur);
    return args.length > 0 && args[0] !== "" ? args : null;
}

// A dump of the session directory: records separated by \u001e, each "<file name>\u001f<text>".
function parseSessionDump(text) {
    var out = [];
    if (typeof text !== "string")
        return out;
    var recs = text.split("\u001e");
    for (var i = 0; i < recs.length; i++) {
        var sep = recs[i].indexOf("\u001f");
        if (sep <= 0)
            continue;
        out.push({ file: recs[i].slice(0, sep), text: recs[i].slice(sep + 1) });
    }
    return out;
}

// [{id, name, command}] from [{file, text}]: valid entries only, uQ first, then by name.
function sessionList(files) {
    var seen = {};
    var list = [];
    for (var i = 0; i < (files || []).length && list.length < MAX_SESSIONS; i++) {
        var f = files[i];
        if (!f || !validSessionId(f.file) || seen[f.file])
            continue;
        var e = parseDesktopEntry(f.text);
        var argv = e ? splitExec(e.exec) : null;
        if (!argv)
            continue;
        seen[f.file] = true;
        list.push({ id: f.file, name: e.name !== "" ? e.name : f.file, command: argv });
    }
    list.sort(function (a, b) {
        var ua = isUqSession(a), ub = isUqSession(b);
        if (ua !== ub)
            return ua ? -1 : 1;
        var n = a.name.toLowerCase().localeCompare(b.name.toLowerCase());
        return n !== 0 ? n : (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
    });
    return list;
}

function isUqSession(s) {
    return !!s && (s.id === UQ_SESSION_ID || (s.command && s.command[0] === UQ_SESSION_EXEC));
}

// The index of the session to preselect for `user`: their last one if it still exists, else uQ
// (first), else 0. -1 for an empty list.
function sessionIndexFor(sessions, st, user) {
    if (!sessions || sessions.length === 0)
        return -1;
    var last = st && st.sessions ? st.sessions[user] : undefined;
    for (var i = 0; last && i < sessions.length; i++)
        if (sessions[i].id === last)
            return i;
    return 0;
}

// The environment a session is launched with.
function launchEnv() {
    return ["XDG_SESSION_TYPE=wayland"];
}

// --- users (AccountsService through `busctl --system --json=short`) ---
function validUserName(n) {
    return typeof n === "string" && /^[A-Za-z_][A-Za-z0-9_.-]{0,31}\$?$/.test(n);
}

function _json(text) {
    try {
        return JSON.parse(text);
    } catch (e) {
        return null;
    }
}

// `busctl call … ListCachedUsers` → the user object paths.
function parseUserPaths(text) {
    var o = _json(text);
    var data = o && Array.isArray(o.data) ? o.data[0] : null;
    if (!Array.isArray(data))
        return [];
    var out = [];
    for (var i = 0; i < data.length && out.length < MAX_USERS; i++)
        if (typeof data[i] === "string" && /^\/org\/freedesktop\/Accounts\/User[0-9]+$/.test(data[i]))
            out.push(data[i]);
    return out;
}

function _prop(props, key, type) {
    var p = props[key];
    return p && typeof p === "object" && p.type === type ? p.data : undefined;
}

// An avatar path is used only when it's a plain file directly in AccountsService's icon folder.
function iconAllowed(path) {
    return typeof path === "string" && path.indexOf(ICONS_DIR) === 0
        && /^[A-Za-z0-9_.$-]+$/.test(path.slice(ICONS_DIR.length))
        && path.slice(ICONS_DIR.length) !== "." && path.slice(ICONS_DIR.length) !== "..";
}

// `busctl call … Properties GetAll s org.freedesktop.Accounts.User` → {name, realName, icon,
// system, locked}, or null when the user name is missing or invalid.
function parseUserProps(text) {
    var o = _json(text);
    var props = o && Array.isArray(o.data) ? o.data[0] : null;
    if (!props || typeof props !== "object")
        return null;
    var name = _prop(props, "UserName", "s");
    if (!validUserName(name))
        return null;
    var real = _prop(props, "RealName", "s");
    var icon = _prop(props, "IconFile", "s");
    return {
        name: name,
        realName: typeof real === "string" ? Notif.cleanText(real, 64) : "",
        icon: iconAllowed(icon) ? icon : "",
        system: _prop(props, "SystemAccount", "b") === true,
        locked: _prop(props, "Locked", "b") === true
    };
}

// The users to list: no system or locked accounts, no duplicates, sorted by shown name. Each gets
// `label` (real name, else user name) and `avatar` (auth.js avatarSource: icon, else initial).
function visibleUsers(users) {
    var seen = {};
    var out = [];
    for (var i = 0; i < (users || []).length; i++) {
        var u = users[i];
        if (!u || u.system || u.locked || !validUserName(u.name) || seen[u.name])
            continue;
        seen[u.name] = true;
        var label = u.realName && u.realName !== "" ? u.realName : u.name;
        out.push({ name: u.name, label: label, avatar: Auth.avatarSource(u.icon, false, "", label) });
    }
    out.sort(function (a, b) {
        var n = a.label.toLowerCase().localeCompare(b.label.toLowerCase());
        return n !== 0 ? n : (a.name < b.name ? -1 : 1);
    });
    return out;
}

// The user to select first: the one whose session just ended early, else the last user, else
// the first in the list. "" for an empty list.
function initialUser(users, st, preferred) {
    var names = (users || []).map(function (u) { return u.name; });
    if (preferred && names.indexOf(preferred) >= 0)
        return preferred;
    if (st && st.lastUser && names.indexOf(st.lastUser) >= 0)
        return st.lastUser;
    return names.length > 0 ? names[0] : "";
}

// --- the greetd conversation ---
// conv: {phase, user, pending, confirm, prompt, messages, message}
//   phase   "idle" | "starting" (createSession sent, no prompt yet) | "prompt" (waiting on the
//           user) | "waiting" (an answer was sent) | "launching" | "launched"
//   pending the password typed before its echo-off prompt arrived (null otherwise)
//   confirm the "Confirm" field of a new password, sent at the retype prompt (null otherwise)
//   prompt  null, or {kind: "secret" | "visible" | "new", text}
//   messages this attempt's info/error views (auth.js messageView), shown if it fails
//   message the one {text, error} on screen
// Each step returns {conv, actions}; actions are {create: user}, {respond: text}, {cancel: true}
// or {launch: true}, applied in order to the Greetd object.

var FAILED_TEXT = "Authentication failed";
var ERROR_TEXT = "The login service reported a problem";
var MISMATCH_TEXT = "The new passwords don't match";
var EMPTY_NEW_TEXT = "Type a new password";

function newConversation() {
    return { phase: "idle", user: "", pending: null, confirm: null, prompt: null, messages: [],
             message: { text: "", error: false } };
}

function _copy(c) {
    return { phase: c.phase, user: c.user, pending: c.pending, confirm: c.confirm,
             prompt: c.prompt, messages: c.messages.slice(), message: c.message };
}

function _idle(c, message) {
    var n = newConversation();
    n.user = c.user;
    n.message = message || { text: "", error: false };
    return n;
}

// Which field an echo-off prompt wants: "confirm" (retype a new password), "new", or "secret".
function classifyPrompt(text) {
    var t = String(text || "").toLowerCase();
    if (/(retype|re-type|re-enter|reenter|again|confirm|repeat|verify)/.test(t))
        return "confirm";
    if (/\bnew\b/.test(t))
        return "new";
    return "secret";
}

// The prompt text as shown: plain, one line, without PAM's trailing colon.
function promptLabel(text, fallback) {
    var t = Notif.cleanText(text, 80).replace(/\s*:\s*$/, "");
    return t !== "" ? t : fallback;
}

// Enter on the main password field with the user selected. An empty field starts the
// conversation without a pending answer (the prompt then waits for the user).
function submit(c, user, typed) {
    if (c.phase === "prompt" && c.prompt && c.prompt.kind !== "new")
        return answer(c, typed);
    if (c.phase !== "idle" || !validUserName(user))
        return { conv: c, actions: [] };
    var n = newConversation();
    n.user = user;
    n.phase = "starting";
    n.pending = typeof typed === "string" && typed !== "" ? typed : null;
    return { conv: n, actions: [{ create: user }] };
}

// The user answered a secret or visible prompt.
function answer(c, text) {
    if (c.phase !== "prompt" || !c.prompt || c.prompt.kind === "new")
        return { conv: c, actions: [] };
    var n = _copy(c);
    n.phase = "waiting";
    n.prompt = null;
    n.message = { text: "", error: false };
    return { conv: n, actions: [{ respond: typeof text === "string" ? text : "" }] };
}

// The user filled in "New password" and "Confirm". A mismatch never reaches PAM.
function answerNew(c, first, second) {
    if (c.phase !== "prompt" || !c.prompt || c.prompt.kind !== "new")
        return { conv: c, actions: [] };
    var n = _copy(c);
    if (typeof first !== "string" || first === "") {
        n.message = { text: EMPTY_NEW_TEXT, error: true };
        return { conv: n, actions: [] };
    }
    if (first !== second) {
        n.message = { text: MISMATCH_TEXT, error: true };
        return { conv: n, actions: [] };
    }
    n.phase = "waiting";
    n.prompt = null;
    n.confirm = second;
    n.message = { text: "", error: false };
    return { conv: n, actions: [{ respond: first }] };
}

// Greetd.authMessage(message, error, responseRequired, echoResponse).
function onAuthMessage(c, text, isError, required, echo) {
    if (c.phase === "idle" || c.phase === "launching" || c.phase === "launched")
        return { conv: c, actions: [] };
    var n = _copy(c);
    if (!required) {
        var v = Auth.messageView(text, isError);
        if (v.text !== "") {
            n.messages.push(v);
            n.message = v;
        }
        return { conv: n, actions: [] };
    }
    if (echo) {
        n.phase = "prompt";
        n.prompt = { kind: "visible", text: promptLabel(text, "Answer") };
        return { conv: n, actions: [] };
    }
    var kind = classifyPrompt(text);
    if (kind === "confirm" && n.confirm !== null) {
        var again = n.confirm;
        n.confirm = null;
        n.phase = "waiting";
        return { conv: n, actions: [{ respond: again }] };
    }
    if (kind === "new") {
        n.pending = null;
        n.phase = "prompt";
        n.prompt = { kind: "new", text: promptLabel(text, "New password") };
        return { conv: n, actions: [] };
    }
    if (n.pending !== null) {
        var typed = n.pending;
        n.pending = null;
        n.phase = "waiting";
        return { conv: n, actions: [{ respond: typed }] };
    }
    n.phase = "prompt";
    n.prompt = { kind: "secret", text: promptLabel(text, "Password") };
    return { conv: n, actions: [] };
}

// Greetd.authFailure(message): greetd's own description is technical, so the attempt's PAM
// messages (e.g. faillock's) are shown, else a plain "Authentication failed".
function onAuthFailure(c) {
    return { conv: _idle(c, Auth.failureView(c.messages, FAILED_TEXT)), actions: [] };
}

// Greetd.error(error): the conversation is over.
function onError(c) {
    return { conv: _idle(c, Auth.failureView(c.messages, ERROR_TEXT)), actions: [] };
}

// Greetd.readyToLaunch(): launch the chosen session.
function onReady(c) {
    if (c.phase === "idle")
        return { conv: c, actions: [] };
    var n = _copy(c);
    n.phase = "launching";
    n.pending = null;
    n.confirm = null;
    n.prompt = null;
    return { conv: n, actions: [{ launch: true }] };
}

function onLaunched(c) {
    var n = _copy(c);
    n.phase = "launched";
    return { conv: n, actions: [] };
}

// Esc, or another user chosen: cancel any conversation in progress so it never blocks the next
// login. `user` is the newly selected user ("" to keep the current one).
function cancel(c, user) {
    // Too late once the session is being started: greetd already has it.
    if (c.phase === "launching" || c.phase === "launched")
        return { conv: c, actions: [] };
    var n = newConversation();
    n.user = typeof user === "string" && user !== "" ? user : c.user;
    return { conv: n, actions: c.phase !== "idle" ? [{ cancel: true }] : [] };
}

// What the field area shows: "password" (the main field), "secret" / "visible" (a PAM prompt),
// "new" (new password and confirm), or "busy".
function fieldMode(c) {
    if (c.phase === "prompt" && c.prompt)
        return c.prompt.kind;
    if (c.phase === "idle")
        return "password";
    return "busy";
}

// --- power ---
var POWER_ACTIONS = ["suspend", "reboot", "poweroff"];
var POWER_LABELS = { suspend: "Suspend", reboot: "Restart", poweroff: "Shut down" };
var POWER_CONFIRM_LABELS = { suspend: "Click again to suspend", reboot: "Click again to restart",
                             poweroff: "Click again to shut down" };

// armed: {action, at} or null. A first click arms; a second click on the same action within
// POWER_CONFIRM_MS runs it. Returns {armed, run}.
function powerClick(armed, action, nowMs) {
    if (POWER_ACTIONS.indexOf(action) < 0)
        return { armed: null, run: "" };
    if (armed && armed.action === action && nowMs - armed.at >= 0 && nowMs - armed.at <= POWER_CONFIRM_MS)
        return { armed: null, run: action };
    return { armed: { action: action, at: nowMs }, run: "" };
}

function powerLabel(armed, action, nowMs) {
    if (armed && armed.action === action && nowMs - armed.at <= POWER_CONFIRM_MS)
        return POWER_CONFIRM_LABELS[action];
    return POWER_LABELS[action] || "";
}

// The logind request, through systemctl (polkit lets the active local session do it).
function powerCommand(action) {
    return POWER_ACTIONS.indexOf(action) >= 0 ? ["systemctl", action] : null;
}

// A refused power request, as plain text.
function powerFailure(action, stderr) {
    var why = Notif.cleanText(String(stderr || "").split("\n")[0], 120);
    var what = { suspend: "Couldn't suspend", reboot: "Couldn't restart", poweroff: "Couldn't shut down" }[action]
        || "Couldn't do that";
    return { text: why !== "" ? what + ": " + why : what, error: true };
}

// --- screens ---
// The monitor that shows the login form: the focused one (the cursor's), else the first.
function mainScreen(names, focused) {
    if (!names || names.length === 0)
        return "";
    return names.indexOf(focused) >= 0 ? focused : names[0];
}
