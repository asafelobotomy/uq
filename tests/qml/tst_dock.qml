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

    function test_class_entries_not_pinnable_or_launchable() {
        var e = Dock.entries([], [tl("0x1", "weird.app")], lookup)[0];
        compare(Dock.canPin(e), false);
        compare(Dock.launchId(e), null);
        compare(Dock.togglePin([], e.id, true), null);
        var k = Dock.entries(["code"], [], lookup)[0];
        compare(Dock.canPin(k), true);
        compare(Dock.launchId(k), "code");
    }
    function test_toggle_pin() {
        compare(JSON.stringify(Dock.togglePin(["a"], "b", true)), '["a","b"]');
        compare(JSON.stringify(Dock.togglePin(["a", "b"], "a", true)), '["b"]');
        compare(JSON.stringify(Dock.togglePin(["a", "a", "b"], "c", true)), '["a","b","c"]');
    }
    function test_toggle_pin_requires_loaded() {
        compare(Dock.togglePin(["a"], "b", false), null);
        compare(Dock.togglePin(undefined, "b", true), null);
    }
    function test_toggle_pin_caps_at_32() {
        var pins = [];
        for (var i = 0; i < 32; i++) pins.push("app" + i);
        compare(Dock.togglePin(pins, "extra", true), null);
        compare(Dock.togglePin(pins, "app0", true).length, 31);
    }

    // The app menu island shows pinned apps with no window open; open ones are on Open windows.
    function test_launchers_are_pinned_apps_without_windows() {
        var es = Dock.entries(["a.desktop", "b.desktop", "c.desktop"], [
            { address: "0x1", cls: "a", minimised: false },
            { address: "0x2", cls: "z", minimised: false },
            { address: "0x3", cls: "c", minimised: true }
        ], function (cls) { return cls === "z" ? null : cls + ".desktop"; });
        compare(Dock.launchers(es).map(function (e) { return e.id; }), ["b.desktop"]);
        compare(Dock.launchers([]), []);
        compare(Dock.launchers(null), []);
    }
}
