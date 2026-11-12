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

    function win(addr, over) {
        var w = { address: addr, cls: "kitty", title: "t" + addr, workspace: "1", minimised: false, monitor: "eDP-1" };
        for (var k in over)
            w[k] = over[k];
        return w;
    }
    function test_taskView_one_tab_per_window() {
        var v = Work.taskView([win("a1"), win("a2"), win("a3")], "eDP-1", "0xa2", false);
        compare(v.count, 3);
        compare(v.folded, false);
        compare(v.tabs.map(function (t) { return t.address; }), ["0xa1", "0xa2", "0xa3"]);
        compare(v.tabs[1].focused, true);
        compare(v.tabs[0].focused, false);
    }
    function test_taskView_folds_past_five_until_expanded() {
        var ws = [];
        for (var i = 1; i <= 6; i++)
            ws.push(win("b" + i));
        compare(Work.taskView(ws.slice(0, 5), "", null, false).folded, false);
        compare(Work.taskView(ws, "", null, false).folded, true);
        compare(Work.taskView(ws, "", null, true).folded, false);
        compare(Work.taskView(ws, "", null, true).tabs.length, 6);
    }
    function test_taskView_filters() {
        var v = Work.taskView([
            win("c1", { monitor: "HDMI-A-1" }),
            win("c2", { workspace: "special:scratch" }),
            win("c3", { workspace: "special:minimized", minimised: true }),
            win("zz"),
            null
        ], "eDP-1", null, false);
        compare(v.tabs.map(function (t) { return t.address; }), ["0xc3"]);
        compare(v.tabs[0].minimised, true);
        compare(Work.taskView([win("c1", { monitor: "HDMI-A-1" })], "", null, false).count, 1);
    }
    function test_titleWidth() {
        compare(Work.titleWidth(2000, 1), 800);
        compare(Work.titleWidth(2000, 5), 160);
        compare(Work.titleWidth(100, 5), 48);
        compare(Work.titleWidth(0, 0), 512);
    }
}
