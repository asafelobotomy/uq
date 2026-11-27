.pragma library

// uq-greeter's decisions (spec phase 5 §2): its config and state files, the session list, the
// user list, the greetd conversation, power confirmation and the "session ended straight away"
// check. Pure: greeter/shell.qml owns the files, processes and the Greetd object and feeds their
// output in here, so qmltestrunner can test all of it with fakes.
//
// Nothing here logs. Passwords are only ever returned as a `respond` action.

.import "auth.js" as Auth
.import "notif.js" as Notif
.import "greeterfiles.js" as Files

// From greeterfiles.js.
var CONFIG_PATH = Files.CONFIG_PATH;
var WALLPAPER_PATH = Files.WALLPAPER_PATH;
var SESSIONS_DIR = Files.SESSIONS_DIR;
var STATE_PATH = Files.STATE_PATH;
var ICONS_DIR = Files.ICONS_DIR;
var ENDED_EARLY_MS = Files.ENDED_EARLY_MS;
var POWER_CONFIRM_MS = Files.POWER_CONFIRM_MS;
var MAX_USERS = Files.MAX_USERS;
var parseToml = Files.parseToml;
var tomlString = Files.tomlString;
var greeterConfig = Files.greeterConfig;
var tokensPath = Files.tokensPath;
var parseState = Files.parseState;
var serializeState = Files.serializeState;
var recordLaunch = Files.recordLaunch;
var endedEarly = Files.endedEarly;
var clearLaunch = Files.clearLaunch;
var ENDED_EARLY_TEXT = Files.ENDED_EARLY_TEXT;
var parseDesktopEntry = Files.parseDesktopEntry;
var splitExec = Files.splitExec;
var parseSessionDump = Files.parseSessionDump;
var sessionList = Files.sessionList;
var isUqSession = Files.isUqSession;
var sessionIndexFor = Files.sessionIndexFor;
var launchEnv = Files.launchEnv;
var validUserName = Files.validUserName;

// --- users (AccountsService through `busctl --system --json=short`; validUserName is in
// greeterfiles.js) ---
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
