import QtQuick
import QtTest
import "../../shell/logic/power.js" as P

TestCase {
    name: "power"

    function test_constants() {
        compare(P.COUNTDOWN_S, 30);
        compare(P.ACTIONS, ["lock", "logout", "suspend", "reboot", "poweroff"]);
    }

    function test_needs_confirm() {
        verify(P.needsConfirm("reboot"));
        verify(P.needsConfirm("poweroff"));
        verify(!P.needsConfirm("lock"));
        verify(!P.needsConfirm("logout"));
        verify(!P.needsConfirm("suspend"));
        verify(!P.needsConfirm("bogus"));
        verify(!P.needsConfirm(""));
        verify(!P.needsConfirm(undefined));
        verify(!P.needsConfirm(null));
    }

    function test_start_confirm() {
        var c = P.startConfirm("reboot", 1000);
        compare(c.action, "reboot");
        compare(c.deadline, 31000);
        compare(P.startConfirm("poweroff", 0).deadline, 30000);
        // Only reboot and poweroff have a confirm step.
        compare(P.startConfirm("lock", 1000), null);
        compare(P.startConfirm("rm -rf", 1000), null);
        compare(P.startConfirm("", 1000), null);
        compare(P.startConfirm("reboot", "x"), null);
    }

    // 30 at the start, counting down in whole seconds (ceil), never negative.
    function test_countdown_remaining() {
        var c = P.startConfirm("poweroff", 10000);
        compare(P.remaining(c, 10000), 30);
        compare(P.remaining(c, 10001), 30);
        compare(P.remaining(c, 10999), 30);
        compare(P.remaining(c, 11000), 29);
        compare(P.remaining(c, 38500), 2);
        compare(P.remaining(c, 39999), 1);
        compare(P.remaining(c, 40000), 0);
        compare(P.remaining(c, 99999), 0);
        compare(P.remaining(null, 0), 0);
        compare(P.remaining({ action: "reboot", deadline: "x" }, 0), 0);
    }

    function test_countdown_due() {
        var c = P.startConfirm("reboot", 0);
        verify(!P.due(c, 0));
        verify(!P.due(c, 29999));
        verify(P.due(c, 30000));
        verify(P.due(c, 45000));
        verify(!P.due(null, 99999));
        verify(!P.due(undefined, 99999));
        // Garbage never fires.
        verify(!P.due({ action: "reboot", deadline: "x" }, 99999));
        verify(!P.due({ action: "rm -rf", deadline: 0 }, 99999));
        verify(!P.due({ action: "lock", deadline: 0 }, 99999));
    }

    // A cancelled confirm is never due, however late it is.
    function test_countdown_cancel() {
        var c = P.startConfirm("poweroff", 0);
        c = P.cancel();
        compare(c, null);
        verify(!P.due(c, 30000));
        verify(!P.due(c, 1e12));
        compare(P.remaining(c, 0), 0);
    }

    function test_labels() {
        var r = P.startConfirm("reboot", 0);
        var s = P.startConfirm("poweroff", 0);
        compare(P.countdownLabel(r, 0), "Restarting in 30 s");
        compare(P.countdownLabel(s, 1500), "Shutting down in 29 s");
        compare(P.countdownLabel(null, 0), "");
        compare(P.nowLabel("reboot"), "Restart now");
        compare(P.nowLabel("poweroff"), "Shut down now");
        compare(P.nowLabel("lock"), "");
    }
}
