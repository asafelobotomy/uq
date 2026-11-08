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

    // One counter shared by every launcher surface, strictly increasing across opens and closes,
    // so the bridge never drops a newer surface's Search as stale.
    function test_shared_generation_strictly_increases() {
        var a = Surfaces.nextGeneration();
        var b = Surfaces.nextGeneration();
        verify(b > a, "next > previous");
        compare(Surfaces.generation, b);
        Surfaces.toggle("spotlight");
        Surfaces.close();
        verify(Surfaces.nextGeneration() > b, "still increasing after close");
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

    // Shell IPC: open only opens, close only closes that surface, unknown names and anything
    // while locked are ignored. IPC always opens on the focused monitor (screen and anchor null).
    function test_ipc_open_close_toggle() {
        Surfaces.toggle("appmenu", { name: "DP-2" }, { id: "button" });
        Surfaces.close();
        compare(Surfaces.ipc("open", "spotlight"), "ok");
        compare(Surfaces.open, "spotlight");
        compare(Surfaces.screen, null);
        compare(Surfaces.anchor, null);
        compare(Surfaces.ipc("open", "spotlight"), "ignored");
        compare(Surfaces.open, "spotlight");
        compare(Surfaces.ipc("close", "appmenu"), "ignored");
        compare(Surfaces.open, "spotlight");
        compare(Surfaces.ipc("open", "appmenu"), "ok");
        compare(Surfaces.open, "appmenu");
        compare(Surfaces.ipc("close", "appmenu"), "ok");
        compare(Surfaces.open, "closed");
        compare(Surfaces.ipc("toggle", "appmenu"), "ok");
        compare(Surfaces.open, "appmenu");
        compare(Surfaces.ipc("toggle", "appmenu"), "ok");
        compare(Surfaces.open, "closed");
    }

    function test_ipc_ignores_unknown_names_and_verbs() {
        compare(Surfaces.ipc("open", "bogus"), "ignored");
        compare(Surfaces.ipc("open", ""), "ignored");
        compare(Surfaces.ipc("lock", "spotlight"), "ignored");
        compare(Surfaces.ipc("unlock", "spotlight"), "ignored");
        compare(Surfaces.open, "closed");
    }

    function test_ipc_ignored_while_locked() {
        Surfaces.ipc("open", "spotlight");
        Bridge.locked = true;
        compare(Surfaces.open, "closed");
        compare(Surfaces.ipc("open", "spotlight"), "ignored");
        compare(Surfaces.ipc("toggle", "appmenu"), "ignored");
        compare(Surfaces.ipc("close", "spotlight"), "ignored");
        compare(Surfaces.open, "closed");
    }
}
