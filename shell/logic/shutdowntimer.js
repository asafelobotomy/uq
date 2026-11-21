.pragma library

// "Shut down in…" (quick settings, System tab): the presets, the custom-time parser, the
// countdown text and when to warn. Pure data in, data out; nothing here runs anything.
// Times are ms (Date.now() in QML), passed in so tests need no timers.
//
// A schedule is {deadline: ms, warned: [seconds...]} while a shutdown is pending, else null.
// A cancelled schedule is null, and null is never due.

// The preset buttons: minutes and the words on the button.
var PRESETS = [
    { minutes: 15, label: "15 min" },
    { minutes: 30, label: "30 min" },
    { minutes: 60, label: "1 hour" },
    { minutes: 120, label: "2 hours" }
];

// A custom time is 1 minute to 24 hours.
var MIN_MINUTES = 1;
var MAX_MINUTES = 24 * 60;

// Warnings go out this many seconds before the shutdown, largest first.
var WARN_AT = [300, 60];

function _valid(s) {
    return !!s && typeof s.deadline === "number" && isFinite(s.deadline) && s.deadline > 0;
}

// A schedule `minutes` from now, or null for a time out of range or not a number.
function schedule(minutes, nowMs) {
    var m = Number(minutes), t = Number(nowMs);
    if (typeof minutes !== "number" || !isFinite(m) || m !== Math.floor(m)
            || m < MIN_MINUTES || m > MAX_MINUTES || typeof nowMs !== "number" || !isFinite(t))
        return null;
    // Warnings already inside the window count as sent: a 1-minute timer gets no "5 minutes" one.
    var warned = WARN_AT.filter(function (w) { return w >= m * 60; });
    return { deadline: t + m * 60000, warned: warned };
}

// Whole seconds left (ceil), never negative; 0 for no (or a garbage) schedule.
function remaining(s, nowMs) {
    if (!_valid(s))
        return 0;
    return Math.max(0, Math.ceil((s.deadline - Number(nowMs)) / 1000));
}

// True once the deadline has passed: the caller shuts down (or drops it, when missed).
function due(s, nowMs) {
    return _valid(s) && Number(nowMs) >= s.deadline;
}

// A deadline noticed this long after it passed was slept through (the machine was suspended),
// and is dropped rather than shutting down the moment the machine wakes.
var MISSED_MS = 90 * 1000;

function missed(s, nowMs) {
    return due(s, nowMs) && Number(nowMs) - s.deadline > MISSED_MS;
}

function _pad(n) {
    return n < 10 ? "0" + n : "" + n;
}

// Seconds as a clock: "1:04:09", "4:09", "0:09".
function clock(seconds) {
    var t = Math.max(0, Math.floor(Number(seconds) || 0));
    var h = Math.floor(t / 3600), m = Math.floor((t % 3600) / 60), s = t % 60;
    return h > 0 ? h + ":" + _pad(m) + ":" + _pad(s) : m + ":" + _pad(s);
}

// "Shutting down in 1:04:09", or "" when nothing is pending.
function countdownLabel(s, nowMs) {
    if (!_valid(s))
        return "";
    return "Shutting down in " + clock(remaining(s, nowMs));
}

// The wall-clock time of the shutdown in local time: "at 23:45", or "at 11:45 PM" when the bar
// clock is set to 12-hour (h24 === false; anything else means 24-hour, uQ's default).
function atLabel(s, h24) {
    if (!_valid(s))
        return "";
    var d = new Date(s.deadline);
    if (h24 === false) {
        var h = d.getHours() % 12;
        return "at " + (h === 0 ? 12 : h) + ":" + _pad(d.getMinutes()) + (d.getHours() < 12 ? " AM" : " PM");
    }
    return "at " + _pad(d.getHours()) + ":" + _pad(d.getMinutes());
}

// Minutes from what someone typed: "45", "90m", "1h", "1h30", "1h 30m", "1:30", "2 hours".
// -1 when it isn't a time, or is outside 1 minute to 24 hours.
function parseCustom(text) {
    if (typeof text !== "string")
        return -1;
    var t = text.trim().toLowerCase();
    var m, mins = -1;
    if ((m = /^(\d{1,4})\s*(m|min|mins|minute|minutes)?$/.exec(t)))
        mins = parseInt(m[1], 10);
    else if ((m = /^(\d{1,2}):([0-5]\d)$/.exec(t)))
        mins = parseInt(m[1], 10) * 60 + parseInt(m[2], 10);
    else if ((m = /^(\d{1,2})\s*(h|hr|hrs|hour|hours)(?:\s*(\d{1,2})\s*(m|min|mins|minute|minutes)?)?$/.exec(t)))
        mins = parseInt(m[1], 10) * 60 + (m[3] !== undefined ? parseInt(m[3], 10) : 0);
    if (mins < MIN_MINUTES || mins > MAX_MINUTES)
        return -1;
    return mins;
}

// The warning to send now (seconds before shutdown, from WARN_AT), or 0 for none. The caller
// adds it to s.warned once sent. Only the nearest threshold goes out, so a laptop waking from
// sleep two minutes before the shutdown gets the 1-minute warning, not both.
function warningDue(s, nowMs) {
    if (!_valid(s) || due(s, nowMs))
        return 0;
    var left = remaining(s, nowMs), warned = Array.isArray(s.warned) ? s.warned : [];
    var pick = 0;
    for (var i = 0; i < WARN_AT.length; i++)
        if (left <= WARN_AT[i] && warned.indexOf(WARN_AT[i]) < 0)
            pick = WARN_AT[i];
    return pick;
}

// s with `w` and every larger threshold marked as sent.
function markWarned(s, w) {
    if (!_valid(s))
        return s;
    var warned = WARN_AT.filter(function (x) { return x >= w; });
    var old = Array.isArray(s.warned) ? s.warned : [];
    for (var i = 0; i < old.length; i++)
        if (warned.indexOf(old[i]) < 0)
            warned.push(old[i]);
    return { deadline: s.deadline, warned: warned };
}

// The notification for a warning, given the seconds left: [summary, body].
function warningText(left) {
    var mins = Math.max(1, Math.ceil(Number(left) / 60) || 1);
    return ["Shutting down in " + mins + (mins === 1 ? " minute" : " minutes"),
        "Save your work. Open quick settings, System, to cancel."];
}

// The file kept in $XDG_RUNTIME_DIR so a shell restart keeps the timer: the deadline in ms.
function serialize(s) {
    return _valid(s) ? String(Math.round(s.deadline)) + "\n" : "";
}

// The schedule from that file, or null when it is empty, garbage, already past, or more than
// MAX_MINUTES away (a clock that moved). A shutdown missed while the shell was down never fires
// late at an unexpected moment.
function restore(text, nowMs) {
    if (typeof text !== "string" || !/^\s*\d{1,16}\s*$/.test(text))
        return null;
    var d = parseInt(text.trim(), 10), now = Number(nowMs);
    if (!isFinite(d) || !isFinite(now) || d <= now || d - now > MAX_MINUTES * 60000)
        return null;
    var left = Math.ceil((d - now) / 1000);
    return { deadline: d, warned: WARN_AT.filter(function (w) { return w > left; }) };
}
