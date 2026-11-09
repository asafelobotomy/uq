.pragma library

// Lock-screen logic: fingerprint detection, avatar choice and PAM prompt handling. Pure: the QML
// lock surface owns the processes and PamContext and feeds their output in here.

.import "notif.js" as Notif

var FPRINT_RETRY_MS = 10000;
var MAX_MESSAGE = 200;

// True only for exit 0 with at least one enrolled-finger line (" - #0: right-index-finger").
// "no fingers enrolled", "No devices available", errors and non-zero exits are all false.
function fprintListed(out, exitCode) {
    if (exitCode !== 0 || typeof out !== "string")
        return false;
    return /^\s*-\s*#\d+:/m.test(out);
}

function initialOf(name) {
    if (typeof name !== "string" || name === "")
        return "?";
    var cp = String.fromCodePoint(name.codePointAt(0));
    return cp.toUpperCase();
}

// The AccountsService icon, then ~/.face, then the user's initial.
function avatarSource(accountsIcon, faceExists, home, name) {
    if (typeof accountsIcon === "string" && accountsIcon !== "")
        return { path: "file://" + accountsIcon.split("/").map(encodeURIComponent).join("/") };
    if (faceExists && typeof home === "string" && home !== "")
        return { path: "file://" + (home + "/.face").split("/").map(encodeURIComponent).join("/") };
    return { initial: initialOf(name) };
}

function fprintCanRestart(lastStartMs, nowMs) {
    return nowMs - lastStartMs >= FPRINT_RETRY_MS;
}

// The typed text goes only to an echo-off prompt; anything else gets no reply.
function passwordReply(responseRequired, responseVisible, typed) {
    return responseRequired && !responseVisible ? typed : null;
}

// The fingerprint context aborts when PAM asks it for a response.
function fprintAbortOnPrompt(responseRequired) {
    return !!responseRequired;
}

function messageView(message, isError) {
    var text = Notif.cleanText(message, MAX_MESSAGE);
    return { text: text, error: text !== "" && !!isError };
}
