.pragma library

// Power-menu logic: which actions need a confirm step, and the 30 s countdown that runs the
// action when it reaches 0 (spec §2.13). Pure data in, data out; nothing here runs anything.
// Times are ms (Date.now() in QML), passed in so tests need no timers.
//
// confirm: {action: "reboot"|"poweroff", deadline: ms} while a countdown is pending, else null.
// A cancelled countdown is null, and null is never due.

var COUNTDOWN_S = 30;
// The bridge's Power names (crates/uqctl/src/bridge/power.rs).
var ACTIONS = ["lock", "logout", "suspend", "reboot", "poweroff"];

var COUNTDOWN_LABEL = { reboot: "Restarting in ", poweroff: "Shutting down in " };
var NOW_LABEL = { reboot: "Restart now", poweroff: "Shut down now" };

// reboot and poweroff have a confirm step; lock, logout and suspend run at once.
function needsConfirm(action) {
    return action === "reboot" || action === "poweroff";
}

function _valid(confirm) {
    return !!confirm && needsConfirm(confirm.action) && typeof confirm.deadline === "number"
        && isFinite(confirm.deadline);
}

// The pending countdown for action, or null when the action has no confirm step.
function startConfirm(action, nowMs) {
    var t = Number(nowMs);
    if (!needsConfirm(action) || typeof nowMs !== "number" || !isFinite(t))
        return null;
    return { action: action, deadline: t + COUNTDOWN_S * 1000 };
}

// Whole seconds left (ceil), never negative; 0 for no (or a garbage) countdown.
function remaining(confirm, nowMs) {
    if (!_valid(confirm))
        return 0;
    return Math.max(0, Math.ceil((confirm.deadline - Number(nowMs)) / 1000));
}

// True once the countdown has reached 0: the caller runs confirm.action.
function due(confirm, nowMs) {
    return _valid(confirm) && Number(nowMs) >= confirm.deadline;
}

function cancel() {
    return null;
}

// "Restarting in N s" / "Shutting down in N s", or "" when nothing is pending.
function countdownLabel(confirm, nowMs) {
    if (!_valid(confirm))
        return "";
    return COUNTDOWN_LABEL[confirm.action] + remaining(confirm, nowMs) + " s";
}

// "Restart now" / "Shut down now", or "".
function nowLabel(action) {
    return needsConfirm(action) ? NOW_LABEL[action] : "";
}
