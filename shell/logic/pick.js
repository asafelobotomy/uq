.pragma library

// Pick-mode state machine (spec §5, §10). Pure: PickOverlay.qml owns the windows and the timer and
// sends `PickDone [token, outcome]` for every non-null `reply`. A pick is "active" while `token`
// is set; every function returns the new state, the `reply` (the outcome as a one-item list, or
// null when there is nothing to send) and the `token` the reply belongs to.

var PICK_TIMEOUT_MS = 20000;

function idle() {
    return { token: null };
}

// What uqd's new_token makes: 32 lowercase hex digits.
function validToken(t) {
    return typeof t === "string" && /^[0-9a-f]{32}$/.test(t);
}

function isActive(state) {
    return !!state && state.token !== null && state.token !== undefined;
}

// A PickRequested event. A bad token is ignored (state unchanged). While the session is locked
// the pick is refused with "cancel" and the overlay stays hidden. A newer token replaces an
// older one (uqd allows one pick at a time, so the older one is already dead).
function start(state, token, locked) {
    if (!validToken(token))
        return { state: state || idle(), reply: null, token: null };
    if (locked)
        return { state: idle(), reply: ["cancel"], token: token };
    return { state: { token: token }, reply: ["shown"], token: token };
}

function finish(state, outcome) {
    if (!isActive(state))
        return { state: idle(), reply: null, token: null };
    return { state: idle(), reply: [outcome], token: state.token };
}

function click(state) {
    return finish(state, "click");
}

function escape(state) {
    return finish(state, "cancel");
}

function timeout(state) {
    return finish(state, "cancel");
}
