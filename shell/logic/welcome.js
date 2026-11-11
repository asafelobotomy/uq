.pragma library

// When the shell starts Welcome (Phase 5 spec §5.1–5.2). Pure: shell/session/WelcomeTrigger.qml
// feeds it bridge events and acts on the result.
//
// The bridge's Hello carries welcome_pending (null while uqd isn't up yet, or when uqd is up but
// didn't answer within the bridge's 2 s; then the shell waits and asks again) and welcome_deferred
// (bindle's $XDG_RUNTIME_DIR/uq/welcome.defer, fresher than 30 min). When Welcome is due, the
// shell first waits GRACE_MS and asks again (WelcomeStatus): an installer's first-login wizard,
// started from the session's autostart, gets that long to create the defer file, so the two
// windows never open together. Once an answer is known the decision is final for this shell:
// Welcome starts at most once, and a deferred Welcome is the installer's to open
// (`uqctl welcome`). A restarted shell asks again, but Settings marks Welcome done as soon as it
// shows it, so that answer is "not due".
//
// state: {decided, waiting}. Each step returns {state, action}, where action is "none", "wait"
// (call onTimer after GRACE_MS), "ask" (call WelcomeStatus; feed the reply, or null on an error,
// to onStatus) or "start" (run `uq-settings --welcome`).

var GRACE_MS = 20000;

function initial() {
    return { decided: false, waiting: false };
}

function result(decided, waiting, action) {
    return { state: { decided: decided, waiting: waiting }, action: action };
}

function onEvent(state, name, data) {
    if (state.decided)
        return { state: state, action: "none" };
    if (name === "Hello") {
        var d = data && typeof data === "object" ? data : {};
        if (d.welcome_pending === false)
            return result(true, false, "none");
        if (d.welcome_pending === true && d.welcome_deferred === true)
            return result(true, false, "none");     // the installer opens it when it's done
        if (d.welcome_pending === true && !state.waiting)
            return result(false, true, "wait");
        // uqd is up but was slow to answer before Hello: no UqdUp will follow, so ask again.
        if (d.welcome_pending === null && d.uqd_up === true && !state.waiting)
            return result(false, true, "wait");
        return { state: state, action: "none" };    // unknown: wait for uqd
    }
    if (name === "UqdUp" && !state.waiting)
        return result(false, true, "wait");
    return { state: state, action: "none" };
}

function onTimer(state) {
    if (state.decided || !state.waiting)
        return { state: state, action: "none" };
    return { state: state, action: "ask" };
}

// The WelcomeStatus reply: {pending, deferred}, or null when the call failed.
function onStatus(state, status) {
    if (state.decided)
        return { state: state, action: "none" };
    if (!status || typeof status.pending !== "boolean")
        return result(false, false, "none");        // still unknown: the next UqdUp waits again
    var start = status.pending === true && status.deferred !== true;
    return result(true, false, start ? "start" : "none");
}
