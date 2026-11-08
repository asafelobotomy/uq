import QtQuick
import QtTest
import "../../shell/logic/dock.js" as Dock

TestCase {
    name: "dock"

    function lookup(cls) {
        var m = { firefox: "firefox", Alacritty: "Alacritty", code: "code" };
        return m[cls] || null;
    }
    function tl(addr, cls, min) {
        return { address: addr, cls: cls, title: "t", workspace: 1, minimised: !!min, monitor: "DP-1" };
    }

    function test_pinned_order_kept() {
        var e = Dock.entries(["code", "firefox"], [], lookup);
        compare(e.length, 2);
        compare(e[0].id, "code");
        compare(e[1].id, "firefox");
        compare(e[0].pinned, true);
        compare(e[0].running, false);
        compare(e[0].windows.length, 0);
    }
    function test_running_unpinned_appended() {
        var e = Dock.entries(["code"], [tl("0x1", "firefox"), tl("0x2", "code")], lookup);
        compare(e.length, 2);
        compare(e[0].id, "code");
        compare(e[0].running, true);
        compare(e[1].id, "firefox");
        compare(e[1].pinned, false);
        compare(e[1].running, true);
    }
    function test_same_class_grouped() {
        var e = Dock.entries([], [tl("0x1", "firefox"), tl("0x2", "Alacritty"), tl("0x3", "firefox")], lookup);
        compare(e.length, 2);
        compare(e[0].id, "firefox");
        compare(JSON.stringify(e[0].windows), '["0x1","0x3"]');
        compare(e[1].id, "Alacritty");
    }
    function test_minimised_only_flag() {
        var e = Dock.entries([], [tl("0x1", "firefox", true), tl("0x2", "code", true), tl("0x3", "code", false)], lookup);
        compare(e[0].minimisedOnly, true);
        compare(e[1].minimisedOnly, false);
        compare(e[0].running, true);
    }
    function test_unknown_class_gets_class_id() {
        var e = Dock.entries([], [tl("0x1", "weird.app")], lookup);
        compare(e[0].id, "class:weird.app");
    }
    function test_empty_class_skipped() {
        compare(Dock.entries([], [tl("0x1", "")], lookup).length, 0);
    }
    function test_duplicate_pins_collapsed() {
        compare(Dock.entries(["code", "code"], [], lookup).length, 1);
    }
    function test_next_window_cycles() {
        var e = Dock.entries([], [tl("0xa", "code"), tl("0xb", "code"), tl("0xc", "code")], lookup)[0];
        compare(Dock.nextWindow(e, ["0xa"]), "0xb");
        compare(Dock.nextWindow(e, ["0xb"]), "0xc");
        compare(Dock.nextWindow(e, ["0xc"]), "0xa");
        compare(Dock.nextWindow(e, ["0xzz"]), "0xa");
        compare(Dock.nextWindow(e, []), "0xa");
    }
    function test_next_window_prefers_visible() {
        var e = Dock.entries([], [tl("0xa", "code"), tl("0xb", "code", true), tl("0xc", "code")], lookup)[0];
        compare(Dock.nextWindow(e, ["0xa"]), "0xc");
    }
}
