.pragma library

// Shared by the lock screen (lock/) and the greeter (greeter/): how PAM messages, failures and
// avatars are shown, and which prompt a typed password answers. Pure: no QML, no I/O.

.import "notif.js" as Notif

var MAX_MESSAGE = 200;

// How many of an attempt's PAM messages a failure shows (pam_faillock sends two).
var FAILURE_LINES = 2;

function initialOf(name) {
    if (typeof name !== "string" || name === "")
        return "?";
    var cp = String.fromCodePoint(name.codePointAt(0));
    return cp.toUpperCase();
}

function fileUrl(path) {
    return "file://" + path.split("/").map(encodeURIComponent).join("/");
}

// The AccountsService icon, then ~/.face, then the user's initial.
function avatarSource(accountsIcon, faceExists, home, name) {
    if (typeof accountsIcon === "string" && accountsIcon !== "")
        return { path: fileUrl(accountsIcon) };
    if (faceExists && typeof home === "string" && home !== "")
        return { path: fileUrl(home + "/.face") };
    return { initial: initialOf(name) };
}

// The typed text goes only to an echo-off prompt; anything else gets no reply.
function passwordReply(responseRequired, responseVisible, typed) {
    return responseRequired && !responseVisible ? typed : null;
}

// A PAM message as plain, bounded, single-line text; `error` only for a non-empty error.
function messageView(message, isError) {
    var text = Notif.cleanText(message, MAX_MESSAGE);
    return { text: text, error: text !== "" && !!isError };
}

// What a failed attempt shows: the attempt's last non-prompt PAM messages (messageView results,
// oldest first), e.g. pam_faillock's "The account is locked due to 3 failed logins." and
// "(10 minutes left to unlock)"; else `fallback` ("Authentication failed"). Always an error.
function failureView(views, fallback) {
    var texts = [];
    var list = Array.isArray(views) ? views : [];
    for (var i = 0; i < list.length; i++) {
        var v = list[i];
        var t = v && typeof v.text === "string" ? Notif.cleanText(v.text, MAX_MESSAGE) : "";
        if (t !== "" && texts[texts.length - 1] !== t)
            texts.push(t);
    }
    var text = Notif.cleanText(texts.slice(-FAILURE_LINES).join(" "), MAX_MESSAGE);
    if (text === "")
        text = typeof fallback === "string" && fallback !== "" ? fallback : "Authentication failed";
    return { text: text, error: true };
}
