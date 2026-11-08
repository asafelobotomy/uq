import QtQuick
import QtTest
import "../../shell/core"

TestCase {
    name: "surfaces"

    SignalSpy { id: wipeSpy; target: Surfaces; signalName: "wipe" }

    function init() {
        Bridge.locked = false;
        Surfaces.close();
        wipeSpy.clear();
    }

    function cleanup() {
        Bridge.locked = false;
        Surfaces.close();
    }

    function test_one_surface_at_a_time() {
        var scr = { name: "DP-1" };
        Surfaces.toggle("spotlight", scr, null);
        compare(Surfaces.open, "spotlight");
        compare(Surfaces.screen, scr);
        compare(wipeSpy.count, 0);
        Surfaces.toggle("appmenu", null, null);
        compare(Surfaces.open, "appmenu");
        compare(wipeSpy.count, 1);
        compare(wipeSpy.signalArguments[0][0], "spotlight");
        Surfaces.toggle("appmenu");
        compare(Surfaces.open, "closed");
        compare(wipeSpy.signalArguments[1][0], "appmenu");
        Surfaces.toggle("bogus");
        compare(Surfaces.open, "closed");
    }

    function test_lock_closes_wipes_and_blocks_toggle() {
        Surfaces.toggle("spotlight");
        Bridge.locked = true;
        compare(Surfaces.open, "closed");
        compare(wipeSpy.count, 2);   // both surfaces wipe on lock
        Surfaces.toggle("spotlight");
        compare(Surfaces.open, "closed");
        Bridge.locked = false;
        Surfaces.toggle("appmenu");
        compare(Surfaces.open, "appmenu");
        Surfaces.lock();
        compare(Surfaces.open, "closed");
    }
}
