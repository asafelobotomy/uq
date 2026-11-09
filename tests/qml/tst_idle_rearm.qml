import QtQuick
import QtTest
import "../../shell/idle"

// Quickshell 0.3.2's IdleMonitor reads `timeout` only when it becomes enabled, so Rearm drops
// `armed` for one turn whenever the timeout changes (L9/L10/L11 in vm-live-check-results.md).
TestCase {
    name: "idle_rearm"

    Rearm { id: r }
    SignalSpy { id: armedSpy; target: r; signalName: "armedChanged" }

    function init() {
        r.want = false;
        r.timeout = 0;
        wait(0);
        armedSpy.clear();
    }

    function test_armed_follows_want() {
        r.timeout = 240;
        wait(0);
        compare(r.armed, false);
        r.want = true;
        compare(r.armed, true);
        r.want = false;
        compare(r.armed, false);
    }

    function test_new_timeout_rearms() {
        r.timeout = 240;
        r.want = true;
        tryCompare(r, "armed", true);
        armedSpy.clear();
        r.timeout = 30;
        compare(r.armed, false, "disabled at once, so the monitor lets go of the old timeout");
        tryCompare(r, "armed", true);
        compare(armedSpy.count, 2, "off, then on with the new timeout");
    }

    function test_same_timeout_does_not_rearm() {
        r.timeout = 240;
        r.want = true;
        tryCompare(r, "armed", true);
        armedSpy.clear();
        r.timeout = 240;
        wait(10);
        compare(armedSpy.count, 0);
    }

    // Saved timings arriving from uqd after startup: the monitor was enabled with the defaults.
    function test_late_settings_apply() {
        r.timeout = 240;
        r.want = true;
        tryCompare(r, "armed", true);
        r.timeout = 30;
        r.want = true;
        tryCompare(r, "armed", true);
        compare(r.timeout, 30);
    }

    // Locking disables the stage; a timeout changed while locked doesn't arm it.
    function test_rearm_respects_want() {
        r.timeout = 240;
        r.want = true;
        tryCompare(r, "armed", true);
        r.want = false;
        r.timeout = 30;
        wait(10);
        compare(r.armed, false);
        r.want = true;
        compare(r.armed, true);
    }
}
