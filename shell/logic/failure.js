.pragma library

// What the shell shows when a bridge call it made for the user fails (a launch, a theme or
// layout switch): `what` ("Couldn't open Firefox") and why, in one short plain line. The reason
// comes from the bridge or uqd; control and direction characters become spaces, and it is cut
// to MAX_REASON.

var MAX_REASON = 120;

// Control characters, line and paragraph separators and direction marks/overrides.
function _unsafe(c) {
    return c < 0x20 || (c >= 0x7f && c <= 0x9f) || c === 0x200e || c === 0x200f
        || (c >= 0x2028 && c <= 0x202e) || (c >= 0x2066 && c <= 0x2069);
}

function _plain(s) {
    var out = "";
    for (var i = 0; i < s.length; i++)
        out += _unsafe(s.charCodeAt(i)) ? " " : s.charAt(i);
    return out.replace(/ +/g, " ").trim();
}

function failureText(what, e) {
    var why = typeof e === "string" ? e : (e && typeof e.message === "string" ? e.message : "");
    why = _plain(why);
    if (why === "")
        return what + ".";
    if (/uqd is not running|bridge|d-?bus/i.test(why) || why === "reset")
        return what + ": uQ's settings service isn't reachable.";
    if (why.length > MAX_REASON)
        why = why.slice(0, MAX_REASON - 1) + "…";
    return what + ": " + why;
}
