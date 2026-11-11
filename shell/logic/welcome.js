.pragma library

// When the shell starts Welcome (Phase 5 spec §5.1–5.2). Pure: shell/session/WelcomeTrigger.qml
// feeds it bridge events and acts on the result.
//
// The bridge's Hello carries welcome_pending (null while uqd isn't up yet) and welcome_deferred
// (bindle's $XDG_RUNTIME_DIR/uq/welcome.defer, fresher than 30 min). Once one answer is known
// the decision is final for this shell: Welcome starts at most once, and a deferred Welcome is
// bindle's to open (`uqctl welcome`). A restarted shell asks again, but Settings marks Welcome
// done as soon as it shows it, so that answer is "not pending".
//
// state: {decided: bool, asked: bool}. Each step returns {state, action}, where action is
// "none", "ask" (call WelcomeStatus; feed the reply to onStatus) or "start" (run
// `uq-settings --welcome`).

function initial() {
    return { decided: false, asked: false };
}

function decide(state, status) {
    if (!status || typeof status.pending !== "boolean")
        return { state: state, action: "none" };       // unknown: wait for uqd
    var start = status.pending === true && status.deferred !== true;
    return { state: { decided: true, asked: state.asked }, action: start ? "start" : "none" };
}

function onEvent(state, name, data) {
    if (state.decided)
        return { state: state, action: "none" };
    if (name === "Hello") {
        var d = data && typeof data === "object" ? data : {};
        return decide(state, { pending: d.welcome_pending, deferred: d.welcome_deferred });
    }
    if (name === "UqdUp" && !state.asked)
        return { state: { decided: false, asked: true }, action: "ask" };
    return { state: state, action: "none" };
}

// The WelcomeStatus reply ({pending, deferred}); a failed call is null.
function onStatus(state, status) {
    if (state.decided)
        return { state: state, action: "none" };
    var r = decide(state, status);
    // Still unknown (uqd went away again): the next UqdUp may ask once more.
    if (!r.state.decided)
        r.state = { decided: false, asked: false };
    return r;
}
