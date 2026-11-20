import QtQuick
import QtTest
import "../../shell/logic/shutdowntimer.js" as T

TestCase {
    name: "shutdowntimer"

    function test_presets() {
        compare(T.PRESETS.map(function (p) { return p.minutes; }), [15, 30, 60, 120]);
        compare(T.PRESETS.map(function (p) { return p.label; }), ["15 min", "30 min", "1 hour", "2 hours"]);
    }

    function test_schedule() {
        var s = T.schedule(15, 1000);
        compare(s.deadline, 1000 + 15 * 60000);
        compare(s.warned, []);
        // Warnings already inside the window count as sent.
        compare(T.schedule(5, 0).warned, [300]);
        compare(T.schedule(1, 0).warned, [300, 60]);
        compare(T.schedule(1440, 0).deadline, 1440 * 60000);
        compare(T.schedule(0, 0), null);
        compare(T.schedule(1441, 0), null);
        compare(T.schedule(1.5, 0), null);
        compare(T.schedule("15", 0), null);
        compare(T.schedule(15, "x"), null);
        compare(T.schedule(NaN, 0), null);
    }

    function test_remaining_and_due() {
        var s = T.schedule(30, 0);
        compare(T.remaining(s, 0), 1800);
        compare(T.remaining(s, 999), 1800);
        compare(T.remaining(s, 1000), 1799);
        compare(T.remaining(s, 1800000), 0);
        compare(T.remaining(s, 9e9), 0);
        verify(!T.due(s, 1799999));
        verify(T.due(s, 1800000));
        verify(!T.due(null, 1e12));
        compare(T.remaining(null, 0), 0);
        compare(T.remaining({ deadline: "x" }, 0), 0);
    }

    function test_missed() {
        var s = T.schedule(1, 0);
        verify(!T.missed(s, 60000));
        verify(!T.missed(s, 60000 + T.MISSED_MS));
        verify(T.missed(s, 60001 + T.MISSED_MS));
        verify(!T.missed(null, 1e12));
    }

    function test_clock() {
        compare(T.clock(0), "0:00");
        compare(T.clock(9), "0:09");
        compare(T.clock(249), "4:09");
        compare(T.clock(3600), "1:00:00");
        compare(T.clock(3849), "1:04:09");
        compare(T.clock(7200), "2:00:00");
        compare(T.clock(-5), "0:00");
        compare(T.clock("x"), "0:00");
    }

    function test_labels() {
        var s = T.schedule(60, 0);
        compare(T.countdownLabel(s, 0), "Shutting down in 1:00:00");
        compare(T.countdownLabel(s, 1000), "Shutting down in 59:59");
        compare(T.countdownLabel(null, 0), "");
        var d = new Date(2026, 10, 20, 23, 5, 0);
        compare(T.atLabel({ deadline: d.getTime(), warned: [] }), "at 23:05");
        compare(T.atLabel(null), "");
    }

    function test_parse_custom() {
        compare(T.parseCustom("45"), 45);
        compare(T.parseCustom(" 45 "), 45);
        compare(T.parseCustom("90m"), 90);
        compare(T.parseCustom("90 min"), 90);
        compare(T.parseCustom("90 minutes"), 90);
        compare(T.parseCustom("1h"), 60);
        compare(T.parseCustom("2 hours"), 120);
        compare(T.parseCustom("1h30"), 90);
        compare(T.parseCustom("1h 30m"), 90);
        compare(T.parseCustom("1H30M"), 90);
        compare(T.parseCustom("1:30"), 90);
        compare(T.parseCustom("0:05"), 5);
        compare(T.parseCustom("24h"), 1440);
        compare(T.parseCustom("1440"), 1440);
        // Out of range or not a time.
        compare(T.parseCustom("0"), -1);
        compare(T.parseCustom("1441"), -1);
        compare(T.parseCustom("25h"), -1);
        compare(T.parseCustom("1:60"), -1);
        compare(T.parseCustom(""), -1);
        compare(T.parseCustom("soon"), -1);
        compare(T.parseCustom("-5"), -1);
        compare(T.parseCustom("1.5"), -1);
        compare(T.parseCustom("1h30h"), -1);
        compare(T.parseCustom(undefined), -1);
    }

    function test_warnings() {
        var s = T.schedule(10, 0);
        compare(T.warningDue(s, 0), 0);
        compare(T.warningDue(s, 299000), 0);           // 301 s left
        compare(T.warningDue(s, 300000), 300);         // 300 s left
        s = T.markWarned(s, 300);
        compare(s.warned, [300]);
        compare(T.warningDue(s, 300000), 0);
        compare(T.warningDue(s, 540000), 60);
        s = T.markWarned(s, 60);
        compare(T.warningDue(s, 590000), 0);
        compare(T.warningDue(s, 600000), 0);           // due, not a warning
        // Woken two minutes before: only the nearest warning, and it marks the larger one sent.
        var t = T.schedule(10, 0);
        compare(T.warningDue(t, 550000), 60);
        compare(T.markWarned(t, 60).warned.sort(), [300, 60].sort());
        compare(T.warningDue(null, 0), 0);
    }

    function test_warning_text() {
        compare(T.warningText(300), ["Shutting down in 5 minutes",
            "Save your work. Open quick settings, System, to cancel."]);
        compare(T.warningText(60)[0], "Shutting down in 1 minute");
        compare(T.warningText(59)[0], "Shutting down in 1 minute");
        compare(T.warningText(0)[0], "Shutting down in 1 minute");
    }

    function test_serialize_restore() {
        var s = T.schedule(30, 1000);
        compare(T.serialize(s), "1801000\n");
        compare(T.serialize(null), "");
        var r = T.restore("1801000\n", 1000);
        compare(r.deadline, 1801000);
        compare(r.warned, []);
        // Restored inside the warning window: warnings already passed count as sent.
        compare(T.restore("1801000", 1801000 - 200000).warned, [300]);
        compare(T.restore("1801000", 1801000), null);       // already due
        compare(T.restore("1801000", 1801001), null);       // past
        compare(T.restore("", 0), null);
        compare(T.restore("garbage", 0), null);
        compare(T.restore("1e9", 0), null);
        compare(T.restore(String(1440 * 60000 + 1), 0), null);   // too far out
        compare(T.restore(undefined, 0), null);
    }
}
