import QtQuick
import QtTest
import "../../shell/logic/mousehold.js" as Hold

TestCase {
    name: "mousehold"

    function test_kind() {
        compare(Hold.kind("custom", "uq-mouse:begin"), "begin");
        compare(Hold.kind("custom", "uq-mouse:end"), "end");
        // Other custom events, other events with the same text, and odd input are nothing.
        compare(Hold.kind("custom", "something-else"), null);
        compare(Hold.kind("custom", "uq-mouse:begin,0"), null);
        compare(Hold.kind("openwindow", "uq-mouse:begin"), null);
        compare(Hold.kind("custom", ""), null);
        compare(Hold.kind("custom", undefined), null);
        compare(Hold.kind("custom", null), null);
        compare(Hold.kind(undefined, "uq-mouse:end"), null);
    }

    function test_note_remembers_the_window_that_was_full() {
        var s = Hold.fresh();
        s = Hold.note(s, "activewindowv2", "5f9acf20f980");
        s = Hold.note(s, "fullscreen", "0");
        s = Hold.note(s, "activewindowv2", "");
        compare(s.full, "5f9acf20f980");
        // A later window or event doesn't replace it.
        s = Hold.note(s, "activewindowv2", "abc");
        s = Hold.note(s, "fullscreen", "0");
        compare(s.full, "5f9acf20f980");
    }

    function test_note_ignores_a_hold_with_nothing_full() {
        var s = Hold.fresh();
        s = Hold.note(s, "activewindowv2", "5f9acf20f980");
        s = Hold.note(s, "fullscreen", "1");
        s = Hold.note(s, "changefloatingmode", "5f9acf20f980,1");
        compare(s.full, "");
        compare(Hold.note(Hold.fresh(), "fullscreen", "0").full, "");
    }

    function test_note_remembers_the_lifted_window() {
        var s = Hold.fresh();
        s = Hold.note(s, "changefloatingmode", "5f9acf20f980,1");
        compare(s.lifted, "5f9acf20f980");
        // Only the first one, and only a window made floating.
        s = Hold.note(s, "changefloatingmode", "abc,1");
        compare(s.lifted, "5f9acf20f980");
        compare(Hold.note(Hold.fresh(), "changefloatingmode", "abc,0").lifted, "");
        compare(Hold.note(Hold.fresh(), "changefloatingmode", "").lifted, "");
    }

    function test_place_finds_the_screen_and_the_spot() {
        var mons = {
            "A": { x: 0, y: 0, width: 2560, height: 1440, scale: 2 },
            "B": { x: 1280, y: 0, width: 1920, height: 1080, scale: 1 }
        };
        var p = Hold.place({ "A": mons.A }, 100, 50);
        compare(p.screen, "A");
        compare(p.x, 100);
        compare(p.y, 50);
        p = Hold.place(mons, 1300, 10);
        compare(p.screen, "B");
        compare(p.x, 20);
        compare(Hold.place(mons, 5000, 5), null);
        compare(Hold.place(mons, NaN, 5), null);
        compare(Hold.place({}, 1, 1), null);
    }

    function test_address_gets_its_prefix() {
        compare(Hold.address("5f9a"), "0x5f9a");
        compare(Hold.address("0x5f9a"), "0x5f9a");
        compare(Hold.address(""), "");
        compare(Hold.address(undefined), "");
    }
}
