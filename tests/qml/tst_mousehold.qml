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
}
