import QtQuick
import QtTest
import "../../shell/logic/work.js" as Work

TestCase {
    name: "work"
    function test_minimiseFrom() {
        compare(Work.minimiseFrom({ id: 3, name: "3" }), 3);
        compare(Work.minimiseFrom({ id: -98, name: "special:minimized" }), null);
        compare(Work.minimiseFrom({ id: 0, name: "x" }), null);
        compare(Work.minimiseFrom(null), null);
        compare(Work.minimiseFrom({ id: "3" }), null);
        compare(Work.minimiseFrom({ id: 2.5 }), null);
    }
    function test_wsChoices() {
        var c = Work.wsChoices();
        compare(c.length, 10);
        compare(c[0], 1);
        compare(c[9], 10);
    }
    function test_normAddr() {
        compare(Work.normAddr("55d0aa"), "0x55d0aa");
        compare(Work.normAddr("0x55d0aa"), "0x55d0aa");
        compare(Work.normAddr("0xZZ"), null);
        compare(Work.normAddr(""), null);
        compare(Work.normAddr("0x12345678901234567"), null);
    }

    readonly property var tgt: ({ addr: "0xabc", ws: 3 })
    function test_buildDispatch() {
        compare(Work.buildDispatch("close", tgt), ["close", { address: "0xabc" }]);
        compare(Work.buildDispatch("toggle_float", tgt), ["toggle_float", { address: "0xabc" }]);
        compare(Work.buildDispatch("pin", tgt), ["pin", { address: "0xabc" }]);
        compare(Work.buildDispatch("fullscreen", tgt, { mode: "full" }), ["fullscreen", { address: "0xabc", mode: "full" }]);
        compare(Work.buildDispatch("fullscreen", tgt, { mode: "maximized" }), ["fullscreen", { address: "0xabc", mode: "maximized" }]);
        compare(Work.buildDispatch("move", tgt, { workspace: 7 }), ["move", { address: "0xabc", workspace: 7 }]);
        compare(Work.buildDispatch("minimise", tgt), ["minimise", { address: "0xabc", from_ws: 3 }]);
    }
    function test_buildDispatch_null() {
        compare(Work.buildDispatch("close", null), null);
        compare(Work.buildDispatch("close", { addr: "zz!", ws: 1 }), null);
        compare(Work.buildDispatch("close", { addr: "", ws: 1 }), null);
        compare(Work.buildDispatch("minimise", { addr: "0xabc", ws: -98 }), null);
        compare(Work.buildDispatch("minimise", { addr: "0xabc", ws: null }), null);
        compare(Work.buildDispatch("fullscreen", tgt, { mode: "bogus" }), null);
        compare(Work.buildDispatch("fullscreen", tgt), null);
        compare(Work.buildDispatch("move", tgt, { workspace: 11 }), null);
        compare(Work.buildDispatch("move", tgt, { workspace: "3" }), null);
        compare(Work.buildDispatch("move", tgt, { workspace: 0 }), null);
        compare(Work.buildDispatch("evil", tgt), null);
    }
}
