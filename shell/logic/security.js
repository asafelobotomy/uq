.pragma library

// Plain-text helpers for the Security island's popup (`oyst-cli status summary --deep --json`).
// Everything the summary says is cut to length; nothing here is trusted as markup.

function _n(v) {
    if (v === null || v === undefined || v === "")
        return -1;
    var x = Number(v);
    return isFinite(x) && x >= 0 ? x : -1;
}

// "just now", "3 hours ago", "2 days ago" from an age in hours.
function ago(hours) {
    var h = _n(hours);
    if (h < 0)
        return "never";
    if (h < 1)
        return "just now";
    if (h < 48)
        return Math.floor(h) + (Math.floor(h) === 1 ? " hour ago" : " hours ago");
    return Math.floor(h / 24) + " days ago";
}

// The label for a scan profile: "quick" -> "Quick scan".
function profileName(p) {
    var s = String(p || "").replace(/[^a-z]/g, "").slice(0, 16);
    return s === "" ? "Scan" : s.charAt(0).toUpperCase() + s.slice(1) + " scan";
}

// Rows of "what oysterAV knows": [{label, value, state}], state ok | warning | threat | muted.
function facts(s) {
    if (!s)
        return [];
    var rows = [];
    if (s.scanning === true)
        rows.push({ label: "Scan", value: "Running now", state: "muted" });
    var age = _n(s.last_scan_age_hours);
    rows.push({
        label: "Last scan",
        value: age < 0 ? "Never run" : ago(age) + " (" + profileName(s.last_scan_profile).toLowerCase() + ")",
        state: age < 0 ? "warning" : (String(s.last_scan_state) === "failed" ? "warning" : "ok")
    });
    var sig = _n(s.signature_age_hours);
    rows.push({
        label: "Virus signatures",
        value: sig < 0 ? "Not found" : (s.signatures_stale === true ? "Out of date, " + ago(sig) : "Up to date, " + ago(sig)),
        state: sig < 0 || s.signatures_stale === true ? "warning" : "ok"
    });
    var f = _n(s.open_findings);
    rows.push({
        label: "Open findings",
        value: f <= 0 ? "None" : String(Math.floor(f)),
        state: f > 0 ? "threat" : "ok"
    });
    var q = _n(s.quarantine_count);
    rows.push({ label: "Quarantine", value: q <= 0 ? "Empty" : Math.floor(q) + (q === 1 ? " file" : " files"), state: "muted" });
    return rows;
}

var CHECK_NAMES = { firewall: "Firewall", boot_guard: "Boot images" };

// The system checks: [{label, value, state}]; unknown ones (not installed) say so quietly.
function checks(s) {
    var list = s && Array.isArray(s.checks) ? s.checks.slice(0, 8) : [];
    return list.map(function (c) {
        var id = String(c && c.id || "").replace(/[^a-z_]/g, "").slice(0, 24);
        var st = String(c && c.state);
        return {
            label: CHECK_NAMES[id] || "Check",
            value: String(c && c.detail || "").slice(0, 120),
            state: st === "ok" ? "ok" : (st === "warning" ? "warning" : "muted")
        };
    });
}
