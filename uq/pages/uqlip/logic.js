.pragma library

// Pure helpers for the Clipboard page (tested with Node and qmltestrunner).

// The first problem in a command's result, or "" when it worked or was cancelled.
function problemOf(result) {
    if (!result || result.cancelled)
        return "";
    if (result.error)
        return String(result.error);
    return "";
}

// One line about the history from `uqctl clip status --json`.
function describe(status) {
    if (!status)
        return "The clipboard history isn't running.";
    var n = status.count || 0;
    var kept = n === 1 ? "1 item kept" : n + " items kept";
    var pinned = status.pinned > 0 ? ", " + status.pinned + " pinned" : "";
    var state = status.enabled === false ? "Off" : (status.paused ? "Paused" : "Recording");
    return state + ". " + kept + pinned + ".";
}

// The apps list as shown: sorted, no repeats ignoring case.
function apps(config) {
    var seen = {}, out = [];
    ((config && config.privacy && config.privacy.ignore_apps) || []).forEach(function (a) {
        var k = String(a).toLowerCase();
        if (!seen[k]) {
            seen[k] = true;
            out.push(String(a));
        }
    });
    out.sort(function (a, b) { return a.toLowerCase() < b.toLowerCase() ? -1 : 1; });
    return out;
}

// Whether `name` is something the page may pass to the ignore commands (the same rule as page.toml).
function validApp(name) {
    return /^[A-Za-z0-9][A-Za-z0-9._ -]{0,63}$/.test(String(name));
}

// The longest text kept, in KB, as the config stores it (bytes).
function textKb(config) {
    var b = config && config.privacy ? config.privacy.max_text_bytes : 0;
    return Math.max(1, Math.round((b || 1048576) / 1024));
}
