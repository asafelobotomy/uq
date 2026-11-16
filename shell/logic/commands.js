.pragma library

// look=see>do_'s Commands group: things to do, not find. Built here from a fixed list (and the
// profiles uqd reports), so no provider text is ever run. `kind` is how Enter acts on a row:
// "power" calls Power with the action, "profile" calls ApplyProfile / ProfileOff. Rows that end
// the session or the power (`noTop`) never become the top hit, so a half-typed word can't
// log you out on Enter.

var POWER = [
    { id: "power:lock", action: "lock", title: "Lock screen", icon: "system-lock-screen",
      words: ["lock", "screen", "lock screen"], noTop: false },
    { id: "power:suspend", action: "suspend", title: "Suspend", icon: "system-suspend",
      words: ["suspend", "sleep"], noTop: true },
    { id: "power:logout", action: "logout", title: "Log out", icon: "system-log-out",
      words: ["log out", "logout", "sign out", "exit"], noTop: true },
    { id: "power:reboot", action: "reboot", title: "Restart", icon: "system-reboot",
      words: ["restart", "reboot"], noTop: true },
    { id: "power:poweroff", action: "poweroff", title: "Power off", icon: "system-shutdown",
      words: ["power off", "poweroff", "shut down", "shutdown"], noTop: true }
];

function _hit(q, words) {
    // The query is the start of the command's name or of one of its words.
    for (var i = 0; i < words.length; i++)
        if (words[i].indexOf(q) === 0)
            return true;
    return false;
}

// query: the search text; profiles: profileList() rows. At least 3 characters, so one or two
// letters never offer a command. Rows: {id, title, subtitle, icon, score, noTop}.
function search(query, profiles, limit) {
    var q = String(query === undefined || query === null ? "" : query).trim().toLowerCase();
    if (q.length < 3)
        return [];
    var out = [];
    for (var i = 0; i < POWER.length; i++) {
        var c = POWER[i];
        if (_hit(q, c.words))
            out.push({ id: c.id, title: c.title, subtitle: "Command", icon: c.icon,
                       score: 0.8, noTop: c.noTop });
    }
    var ps = Array.isArray(profiles) ? profiles : [];
    for (var j = 0; j < ps.length; j++) {
        var p = ps[j];
        var name = String(p.name || "");
        var words = ["profile", "switch to " + name.toLowerCase(), name.toLowerCase(),
                     "profile " + name.toLowerCase()];
        if (p.active === true || !_hit(q, words))
            continue;
        out.push({ id: "profile:" + p.id, title: "Switch to profile " + name,
                   subtitle: "Command", icon: "preferences-system", score: 0.7, noTop: false });
    }
    if (ps.some(function (p) { return p.active === true; })
            && _hit(q, ["profile off", "back to normal", "profile"]))
        out.push({ id: "profile-off", title: "Back to normal (profile off)",
                   subtitle: "Command", icon: "preferences-system", score: 0.7, noTop: false });
    out.sort(function (a, b) { return b.score - a.score; });
    return out.slice(0, limit === undefined ? 5 : limit);
}

// The bridge call for a Commands row id: {call, args} or null.
function activation(id) {
    var s = String(id);
    for (var i = 0; i < POWER.length; i++)
        if (POWER[i].id === s)
            return { call: "Power", args: [POWER[i].action] };
    if (s === "profile-off")
        return { call: "ProfileOff", args: [] };
    if (s.indexOf("profile:") === 0 && s.length > 8)
        return { call: "ApplyProfile", args: [s.slice(8)] };
    return null;
}
