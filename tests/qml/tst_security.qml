import QtQuick
import QtTest
import "../../shell/logic/security.js" as S

TestCase {
    name: "security"

    function test_ago() {
        compare(S.ago(null), "never");
        compare(S.ago(0.2), "just now");
        compare(S.ago(1), "1 hour ago");
        compare(S.ago(5.9), "5 hours ago");
        compare(S.ago(72), "3 days ago");
    }

    function test_facts_stale() {
        var rows = S.facts({ scanning: false, last_scan_age_hours: 3, last_scan_profile: "quick",
            last_scan_state: "done", open_findings: 2, signature_age_hours: 70, signatures_stale: true,
            quarantine_count: 1 });
        compare(rows.map(function (r) { return r.label; }),
                ["Last scan", "Virus signatures", "Open findings", "Quarantine"]);
        compare(rows[0].value, "3 hours ago (quick scan)");
        compare(rows[1].value, "Out of date, 2 days ago");
        compare(rows[1].state, "warning");
        compare(rows[2].state, "threat");
        compare(rows[3].value, "1 file");
    }

    function test_facts_never_scanned_and_scanning() {
        var rows = S.facts({ scanning: true, last_scan_age_hours: null, signature_age_hours: 2,
            signatures_stale: false, open_findings: 0, quarantine_count: 0 });
        compare(rows[0].value, "Running now");
        compare(rows[1].value, "Never run");
        compare(rows[2].value, "Up to date, 2 hours ago");
        compare(rows[3].value, "None");
    }

    function test_checks_are_cut_and_named() {
        var c = S.checks({ checks: [{ id: "firewall", state: "warning", detail: "x".repeat(300) },
                                    { id: "boot_guard", state: "unknown", detail: "not installed" },
                                    { id: "<b>", state: "ok", detail: "fine" }] });
        compare(c[0].label, "Firewall");
        compare(c[0].value.length, 120);
        compare(c[1].state, "muted");
        compare(c[2].label, "Check");
        compare(S.checks(null), []);
        compare(S.facts(null), []);
    }
}
