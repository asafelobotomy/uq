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
        compare(wipeSpy.count, 5);   // every surface wipes on lock
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

    function test_quicksettings_surface() {
        Surfaces.toggle("spotlight");
        wipeSpy.clear();
        Surfaces.toggle("quicksettings", { name: "DP-1" }, { id: "button" });
        compare(Surfaces.open, "quicksettings");
        compare(wipeSpy.count, 1);
        compare(wipeSpy.signalArguments[0][0], "spotlight");
        Surfaces.toggle("quicksettings");
        compare(Surfaces.open, "closed");
        compare(wipeSpy.signalArguments[1][0], "quicksettings");
        Surfaces.toggle("quicksettings");
        wipeSpy.clear();
        Bridge.locked = true;
        compare(Surfaces.open, "closed");
        var names = [];
        for (var i = 0; i < wipeSpy.count; i++)
            names.push(wipeSpy.signalArguments[i][0]);
        verify(names.indexOf("quicksettings") >= 0, "lock wipes quicksettings");
    }

    function test_ipc_quicksettings() {
        compare(Surfaces.ipc("close", "quicksettings"), "ignored");
        compare(Surfaces.ipc("open", "quicksettings"), "ok");
        compare(Surfaces.open, "quicksettings");
        compare(Surfaces.screen, null);
        compare(Surfaces.anchor, null);
        compare(Surfaces.ipc("open", "quicksettings"), "ignored");
        compare(Surfaces.ipc("close", "spotlight"), "ignored");
        compare(Surfaces.open, "quicksettings");
        compare(Surfaces.ipc("close", "quicksettings"), "ok");
        compare(Surfaces.open, "closed");
        compare(Surfaces.ipc("toggle", "quicksettings"), "ok");
        compare(Surfaces.open, "quicksettings");
        compare(Surfaces.ipc("toggle", "quicksettings"), "ok");
        compare(Surfaces.open, "closed");
        Surfaces.ipc("open", "quicksettings");
        Bridge.locked = true;
        compare(Surfaces.ipc("toggle", "quicksettings"), "ignored");
    }

    // The built-in centre opens through toggle() (NotifBackend, SUPER+N) and wipes on lock, but
    // the shell IPC verbs don't take it: its keybind goes through NotifBackend.
    function test_notifcentre_surface() {
        Surfaces.toggle("spotlight");
        wipeSpy.clear();
        Surfaces.toggle("notifcentre", null, null);
        compare(Surfaces.open, "notifcentre");
        compare(wipeSpy.count, 1);
        compare(wipeSpy.signalArguments[0][0], "spotlight");
        Surfaces.toggle("notifcentre");
        compare(Surfaces.open, "closed");
        compare(wipeSpy.signalArguments[1][0], "notifcentre");
        Surfaces.toggle("notifcentre");
        wipeSpy.clear();
        Bridge.locked = true;
        compare(Surfaces.open, "closed");
        var names = [];
        for (var i = 0; i < wipeSpy.count; i++)
            names.push(wipeSpy.signalArguments[i][0]);
        verify(names.indexOf("notifcentre") >= 0, "lock wipes notifcentre");
        Bridge.locked = false;
        Surfaces.toggle("notifcentre");
        compare(Surfaces.open, "notifcentre");
        Bridge.locked = true;
        Surfaces.toggle("notifcentre");
        compare(Surfaces.open, "closed");
    }

    function test_ipc_rejects_notifcentre() {
        compare(Surfaces.ipc("open", "notifcentre"), "ignored");
        compare(Surfaces.ipc("toggle", "notifcentre"), "ignored");
        compare(Surfaces.open, "closed");
        Surfaces.toggle("notifcentre");
        compare(Surfaces.open, "notifcentre");
        compare(Surfaces.ipc("close", "notifcentre"), "ignored");
        compare(Surfaces.open, "notifcentre");
    }

    // The power menu opens from SUPER+Escape and IPC with no confirm pending, and from the app
    // menu's restart and shut-down buttons at that action's confirm step (Surfaces.powerConfirm).
    function test_powermenu_surface_and_confirm() {
        Surfaces.toggle("appmenu");
        wipeSpy.clear();
        Surfaces.toggle("powermenu", null, null, "reboot");
        compare(Surfaces.open, "powermenu");
        compare(Surfaces.powerConfirm, "reboot");
        compare(wipeSpy.count, 1);
        compare(wipeSpy.signalArguments[0][0], "appmenu");
        Surfaces.toggle("powermenu");
        compare(Surfaces.open, "closed");
        compare(Surfaces.powerConfirm, "");
        compare(wipeSpy.signalArguments[1][0], "powermenu");
        // Only reboot and poweroff are confirm steps.
        Surfaces.toggle("powermenu", null, null, "lock");
        compare(Surfaces.powerConfirm, "");
        Surfaces.close();
        Surfaces.toggle("powermenu", null, null, "poweroff");
        compare(Surfaces.powerConfirm, "poweroff");
        // Another surface replaces it and clears the pending confirm.
        Surfaces.toggle("spotlight");
        compare(Surfaces.powerConfirm, "");
        Surfaces.toggle("powermenu", null, null, "poweroff");
        wipeSpy.clear();
        Bridge.locked = true;
        compare(Surfaces.open, "closed");
        compare(Surfaces.powerConfirm, "");
        var names = [];
        for (var i = 0; i < wipeSpy.count; i++)
            names.push(wipeSpy.signalArguments[i][0]);
        verify(names.indexOf("powermenu") >= 0, "lock wipes powermenu");
        Surfaces.toggle("powermenu", null, null, "reboot");
        compare(Surfaces.open, "closed");
        compare(Surfaces.powerConfirm, "");
    }

    function test_ipc_powermenu() {
        compare(Surfaces.ipc("close", "powermenu"), "ignored");
        compare(Surfaces.ipc("open", "powermenu"), "ok");
        compare(Surfaces.open, "powermenu");
        compare(Surfaces.screen, null);
        compare(Surfaces.anchor, null);
        compare(Surfaces.powerConfirm, "");
        compare(Surfaces.ipc("open", "powermenu"), "ignored");
        compare(Surfaces.ipc("close", "powermenu"), "ok");
        compare(Surfaces.open, "closed");
        compare(Surfaces.ipc("toggle", "powermenu"), "ok");
        compare(Surfaces.open, "powermenu");
        compare(Surfaces.ipc("toggle", "powermenu"), "ok");
        compare(Surfaces.open, "closed");
        Surfaces.ipc("open", "powermenu");
        Bridge.locked = true;
        compare(Surfaces.open, "closed");
        compare(Surfaces.ipc("toggle", "powermenu"), "ignored");
        compare(Surfaces.ipc("open", "powermenu"), "ignored");
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
