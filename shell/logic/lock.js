.pragma library

// Lock-screen logic: fingerprint detection, avatar choice and PAM prompt handling. Pure: the QML
// lock surface owns the processes and PamContext and feeds their output in here. The parts the
// greeter shares (messages, failures, avatars, prompt replies) live in auth.js; they are
// re-exported here unchanged.

.import "auth.js" as Auth

var FPRINT_RETRY_MS = 10000;
var MAX_MESSAGE = Auth.MAX_MESSAGE;

// True only for exit 0 with at least one enrolled-finger line (" - #0: right-index-finger").
// "no fingers enrolled", "No devices available", errors and non-zero exits are all false.
function fprintListed(out, exitCode) {
    if (exitCode !== 0 || typeof out !== "string")
        return false;
    return /^\s*-\s*#\d+:/m.test(out);
}

function initialOf(name) {
    return Auth.initialOf(name);
}

// The AccountsService icon, then ~/.face, then the user's initial.
function avatarSource(accountsIcon, faceExists, home, name) {
    return Auth.avatarSource(accountsIcon, faceExists, home, name);
}

function fprintCanRestart(lastStartMs, nowMs) {
    return nowMs - lastStartMs >= FPRINT_RETRY_MS;
}

// The typed text goes only to an echo-off prompt; anything else gets no reply.
function passwordReply(responseRequired, responseVisible, typed) {
    return Auth.passwordReply(responseRequired, responseVisible, typed);
}

// The fingerprint context aborts when PAM asks it for a response.
function fprintAbortOnPrompt(responseRequired) {
    return !!responseRequired;
}

function messageView(message, isError) {
    return Auth.messageView(message, isError);
}

// How many of an attempt's PAM messages a failure shows (pam_faillock sends two).
var FAILURE_LINES = Auth.FAILURE_LINES;

// What a failed password attempt shows (auth.js failureView): the attempt's last PAM messages,
// e.g. pam_faillock's lockout text, else `fallback`. Always an error.
function failureView(views, fallback) {
    return Auth.failureView(views, fallback);
}

// The shell config next to the lock config: <dir>/../shell, normalised (uq-lock calls the shell's
// `lockSecure` IPC there).
function siblingShellDir(lockDir) {
    return String(lockDir).replace(/\/+$/, "").replace(/\/[^\/]*$/, "") + "/shell";
}

// Recovery: while the session is locked, uq-lock.service must be running. Start it unless it's
// active or a start is already in flight.
function recoveryDecision(locked, unitActive, starting) {
    return locked && !unitActive && !starting ? "start" : "none";
}

// How long a lockSecure that arrives before LockState(true) is remembered, and how recent a Lock
// request must be for such an early lockSecure to count.
var EARLY_SECURE_MS = 5000;

// Sleep and secure state, one event at a time. Times are ms; -1 means "never".
// s:  {locked, secure, sleepPending, earlySecureAt, lockRequestedAt, starting}
// ev: "secure"   uq-lock reported WlSessionLock.secure
//     "locked"   LockState(true)
//     "unlocked" LockState(false)
//     "sleep"    PrepareForSleep(true)
//     "wake"     PrepareForSleep(false)
// Returns {secure, sleepPending, earlySecureAt, sleepReady, lock}: the new state, whether to send
// SleepLockReady now, and whether to start the lock. Nothing here can unlock: `secure` only lets
// a pending sleep go ahead.
function secureStep(s, ev, nowMs) {
    var r = { secure: !!s.secure, sleepPending: !!s.sleepPending, earlySecureAt: s.earlySecureAt,
              sleepReady: false, lock: false };
    if (typeof r.earlySecureAt !== "number")
        r.earlySecureAt = -1;
    var requested = s.starting || (s.lockRequestedAt >= 0 && nowMs - s.lockRequestedAt <= EARLY_SECURE_MS);
    switch (ev) {
    case "secure":
        if (s.locked)
            r.secure = true;
        else if (requested)
            r.earlySecureAt = nowMs;
        break;
    case "locked":
        if (r.earlySecureAt >= 0 && nowMs - r.earlySecureAt <= EARLY_SECURE_MS)
            r.secure = true;
        r.earlySecureAt = -1;
        break;
    case "unlocked":
        r.secure = false;
        r.earlySecureAt = -1;
        break;
    case "sleep":
        r.sleepPending = true;
        r.lock = !r.secure && !s.locked;
        break;
    case "wake":
        r.sleepPending = false;
        break;
    }
    if (r.secure && r.sleepPending) {
        r.sleepReady = true;
        r.sleepPending = false;
    }
    return r;
}
