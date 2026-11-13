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

    // --- Open windows: a pill per window ---

    function wins() {
        return [
            { address: "0xa", cls: "firefox", title: "Docs", wsId: 1, minimised: false, monitor: "DP-1", pid: 10 },
            { address: "0xb", cls: "kitty", title: "shell", wsId: 2, minimised: false, monitor: "DP-1", pid: 11 },
            { address: "0xc", cls: "mpv", title: "film", wsId: -99, minimised: true, minimisedFrom: 1, monitor: "DP-1", pid: 12 },
            { address: "0xd", cls: "gimp", title: "art", wsId: 1, minimised: false, monitor: "HDMI-A-1", pid: 13 },
            { address: "0xe", cls: "kate", title: "notes", wsId: -99, minimised: true, minimisedFrom: 2, monitor: "DP-1", pid: 14 },
            { address: "bad!", cls: "x", title: "", wsId: 1, monitor: "DP-1" }
        ];
    }

    function test_windows_here_are_this_workspace_and_its_minimised() {
        var h = Work.windowsHere(wins(), "DP-1", 1, "0xa");
        compare(h.map(function (w) { return w.address; }), ["0xa", "0xc"]);
        verify(h[0].focused);
        verify(h[1].minimised, "minimised from workspace 1: shown here, dimmed");
        compare(h[0].pid, 10);
        compare(Work.windowsHere(wins(), "DP-1", 2, "").map(function (w) { return w.address; }), ["0xb", "0xe"]);
        compare(Work.windowsHere(wins(), "HDMI-A-1", 1, "").map(function (w) { return w.address; }), ["0xc", "0xd"],
                "a minimised window follows the workspace it left");
        compare(Work.windowsHere(wins(), "DP-1", undefined, ""), []);
    }

    function test_sort_orders() {
        var list = [{ address: "0x1", cls: "zed", title: "b" }, { address: "0x2", cls: "alpha", title: "a" },
                    { address: "0x3", cls: "mid", title: "c" }];
        var ctx = {
            opened: { "0x1": 2, "0x2": 0, "0x3": 1 },
            activity: { "0x3": 500, "0x1": 100 },
            pinIndex: { "0x3": 0, "0x1": 1 },
            usage: { "0x1": 5, "0x2": 50, "0x3": 0.5 },
            name: { "0x1": "Zed", "0x2": "Alpha", "0x3": "Mid" }
        };
        var ids = function (l) { return l.map(function (w) { return w.address; }); };
        compare(ids(Work.sortWindows(list, "opened", false, ctx)), ["0x2", "0x3", "0x1"], "oldest first");
        compare(ids(Work.sortWindows(list, "opened", true, ctx)), ["0x1", "0x3", "0x2"], "newest first");
        compare(ids(Work.sortWindows(list, "activity", false, ctx)), ["0x3", "0x1", "0x2"], "most recent first, none last");
        compare(ids(Work.sortWindows(list, "pin", false, ctx)), ["0x3", "0x1", "0x2"], "pins in pin order, then the rest");
        compare(ids(Work.sortWindows(list, "pin", true, ctx)), ["0x3", "0x1", "0x2"], "pin has one order");
        compare(ids(Work.sortWindows(list, "usage", false, ctx)), ["0x2", "0x1", "0x3"], "busiest first");
        compare(ids(Work.sortWindows(list, "name", false, ctx)), ["0x2", "0x3", "0x1"]);
        compare(ids(Work.sortWindows(list, "name", true, ctx)), ["0x1", "0x3", "0x2"]);
        compare(ids(Work.sortWindows(list, "bogus", false, ctx)), ["0x2", "0x3", "0x1"], "unknown is opening order");
        verify(!Work.sortHasOrder("pin") && Work.sortHasOrder("usage"));
        compare(Work.SORTS.length, 5);
    }

    function test_notification_matches_app() {
        verify(Work.notifMatches({ appName: "Firefox" }, { cls: "firefox", id: "", name: "" }));
        verify(Work.notifMatches({ appName: "Thunderbird", appIcon: "org.mozilla.thunderbird" },
                                 { cls: "thunderbird-esr", id: "org.mozilla.Thunderbird.desktop", name: "Mail" }));
        verify(Work.notifMatches({ appName: "Mail" }, { cls: "x", id: "", name: "Mail" }));
        verify(!Work.notifMatches({ appName: "Slack" }, { cls: "firefox", id: "firefox.desktop", name: "Firefox" }));
        verify(!Work.notifMatches(null, { cls: "x" }));
        verify(!Work.notifMatches({ appName: "" }, { cls: "" }));
    }

    readonly property var m: ({ minPill: 49, maxPill: 180, gap: 2, arrow: 16, chip: 32 })

    function test_strip_shrinks_evenly_then_scrolls() {
        compare(Work.strip(0, 206, m).mode, "empty");
        var one = Work.strip(1, 206, m);
        compare(one.mode, "pills");
        compare(one.pillW, 180, "a lone pill stops at its widest");
        var three = Work.strip(3, 206, m);
        compare(three.pillW, Math.floor((206 - 4) / 3));
        var four = Work.strip(4, 206, m);
        compare(four.mode, "pills");
        compare(four.pillW, 50, "down to just over icon and close");
        var many = Work.strip(9, 206, m);
        compare(many.mode, "scroll");
        verify(many.shown >= 1 && many.shown < 9);
        verify(many.shown * many.pillW + (many.shown - 1) * m.gap + 2 * (m.arrow + m.gap) + m.chip + m.gap <= 206,
               "arrows, chip and pills fit");
        verify(many.pillW >= m.minPill);
        compare(Work.strip(3, 26, m).mode, "compact", "Icon size: the lead window's icon");
        compare(Work.strip(1, 62, m).mode, "pills");
        compare(Work.strip(2, 62, m).mode, "compact");
        // Never wider than the strip.
        for (var w = 20; w <= 400; w += 7)
            for (var n = 1; n <= 12; n++) {
                var f = Work.strip(n, w, m);
                if (f.mode === "pills")
                    verify(n * f.pillW + (n - 1) * m.gap <= w, n + " pills in " + w);
                if (f.mode === "scroll")
                    verify(f.shown * f.pillW + (f.shown - 1) * m.gap + 2 * (m.arrow + m.gap) + m.chip + m.gap <= w);
            }
    }

    function test_scroll_offset_keeps_focus_visible() {
        compare(Work.scrollOffset(0, 9, 3, -1), 0);
        compare(Work.scrollOffset(20, 9, 3, -1), 6, "clamped");
        compare(Work.scrollOffset(0, 9, 3, 5), 3, "moves just enough to show it");
        compare(Work.scrollOffset(6, 9, 3, 1), 1);
        compare(Work.scrollOffset(2, 9, 3, 3), 2, "already shown");
    }
}
