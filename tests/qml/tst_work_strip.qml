import QtQuick
import QtTest
import "../../shell/logic/work.js" as Work

// The open-windows strip's logic (logic/work.js): widths, tab sizes, scrolling, dragging, the
// close button, and the attention dots. The windows themselves are in tst_work.qml.
TestCase {
    name: "work_strip"
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

    function test_screen_order() {
        var ids = function (l) { return l.map(function (w) { return w.address; }); };
        var list = [
            { address: "0x1", title: "", x: 900, y: 0 },
            { address: "0x2", title: "", x: -500, y: 0 },
            { address: "0x3", title: "", x: 900, y: -300 },
            { address: "0x4", title: "", x: 2000, y: 0 }
        ];
        compare(ids(Work.sortWindows(list, "screen", false, {})), ["0x2", "0x3", "0x1", "0x4"],
                "left to right, top to bottom in a column");
        compare(ids(Work.sortWindows(list, "screen", true, {})), ["0x2", "0x3", "0x1", "0x4"], "one order");
        // 0x3 is minimised: it keeps its place after 0x2, where it was last shown.
        var mins = [
            { address: "0x1", title: "", x: 900 },
            { address: "0x2", title: "", x: -500 },
            { address: "0x3", title: "", minimised: true },
            { address: "0x4", title: "", x: 1500 },
            { address: "0x5", title: "", minimised: true }
        ];
        compare(ids(Work.screenOrder(mins, ["0x2", "0x3", "0x1", "0x4"])), ["0x2", "0x3", "0x1", "0x4", "0x5"],
                "minimised windows keep their slot; one never shown goes last");
        compare(ids(Work.screenOrder(mins, ["0x3", "0x2"])), ["0x3", "0x2", "0x1", "0x4", "0x5"], "first stays first");
        compare(ids(Work.screenOrder(mins, ["0x2", "0x3", "0x5", "0x1"])), ["0x2", "0x3", "0x5", "0x1", "0x4"],
                "two minimised in a row stay in order");
    }

    readonly property var mt: ({ minPill: 28, maxPill: 180, gap: 2, arrow: 16, chip: 32, medium: 80 })

    function test_strip_focused_tab_is_medium() {
        var all = Work.strip(5, 206, mt, 2);
        compare(all.mode, "pills");
        compare(all.focusW, 80, "the focused tab grows to Medium");
        compare(all.pillW, Math.floor((206 - 80 - 4 * 2) / 4), "the others share the rest");
        var roomy = Work.strip(2, 400, mt, 0);
        compare(roomy.focusW, roomy.pillW, "already Medium or wider: all even");
        var tight = Work.strip(6, 6 * 28 + 5 * 2 + 20, mt, 0);
        compare(tight.mode, "pills", "all icons still fit: the focused tab takes what is left");
        compare(tight.pillW, 28);
        compare(tight.focusW, 48);
        var many = Work.strip(20, 300, mt, 7);
        compare(many.mode, "scroll");
        compare(many.focusW, 80);
        verify(many.shown >= 2);
        verify(many.focusW + (many.shown - 1) * (many.pillW + mt.gap) + 2 * (mt.arrow + mt.gap) + mt.chip + mt.gap <= 300);
        verify(many.pillW >= mt.minPill);
        compare(Work.strip(3, 20, mt, 0).mode, "compact");
        for (var w = 20; w <= 400; w += 7)
            for (var n = 1; n <= 12; n++)
                for (var f = 0; f < n; f += 3) {
                    var s = Work.strip(n, w, mt, f);
                    if (s.mode === "pills")
                        verify(s.focusW + (n - 1) * (s.pillW + mt.gap) <= w, n + " pills in " + w);
                    if (s.mode === "scroll")
                        verify(s.focusW + (s.shown - 1) * (s.pillW + mt.gap) + 2 * (mt.arrow + mt.gap) + mt.chip + mt.gap <= w);
                }
    }

    function test_spans_screen() {
        var mon = { width: 3840, scale: 2, reserved: [0, 60, 0, 0] };
        verify(Work.spansScreen({ size: [1912, 1004] }, mon), "a Full column");
        verify(!Work.spansScreen({ size: [932, 1004] }, mon), "a Half column");
        verify(Work.spansScreen({ size: [1300, 1004] }, { width: 1920, scale: 1, reserved: [400, 0, 0, 0] }), "reserved space is left out");
        verify(!Work.spansScreen({}, mon));
        verify(!Work.spansScreen({ size: [1912, 10] }, null));
    }

    function test_tab_tier() {
        var t = { small: 60, medium: 80, large: 130 };
        compare(Work.tabTier(28, t), "icon");
        compare(Work.tabTier(60, t), "small");
        compare(Work.tabTier(100, t), "medium");
        compare(Work.tabTier(130, t), "large");
    }

    function test_centre_offset() {
        compare(Work.centreOffset(9, 3, 5), 4);
        compare(Work.centreOffset(9, 3, 0), 0);
        compare(Work.centreOffset(9, 3, 8), 6, "clamped");
        compare(Work.centreOffset(9, 4, 5), 4);
    }

    function test_scroll_offset_keeps_focus_visible() {
        compare(Work.scrollOffset(0, 9, 3, -1), 0);
        compare(Work.scrollOffset(20, 9, 3, -1), 6, "clamped");
        compare(Work.scrollOffset(0, 9, 3, 5), 3, "moves just enough to show it");
        compare(Work.scrollOffset(6, 9, 3, 1), 1);
        compare(Work.scrollOffset(2, 9, 3, 3), 2, "already shown");
    }

    function test_column_steps() {
        var a = { address: "0xa", x: 0 }, b = { address: "0xb", x: 500 }, b2 = { address: "0xb2", x: 500 },
            c = { address: "0xc", x: 1000 }, m = { address: "0xm", minimised: true, x: 0 };
        compare(Work.columnSteps([a, b, c], 0, 2), 2, "first to last: two columns right");
        compare(Work.columnSteps([a, b, c], 2, 0), -2);
        compare(Work.columnSteps([a, b, c], 1, 1), 0);
        compare(Work.columnSteps([a, b, b2, c], 0, 3), 2, "a stacked column counts once");
        compare(Work.columnSteps([a, b, b2, c], 0, 1), 1, "dropped inside a stack: after the whole column");
        compare(Work.columnSteps([a, b, b2, c], 0, 2), 1, "past the whole stack");
        compare(Work.columnSteps([a, m, b, c], 0, 2), 1, "a minimised window is no column");
        compare(Work.columnSteps([a, b], 5, 0), 0);
    }

    function test_drop_index_and_can_drag() {
        var lefts = [0, 100, 200, 300], widths = [90, 90, 90, 90];
        compare(Work.dropIndex(lefts, widths, 0, 30), 0, "not past the next centre");
        compare(Work.dropIndex(lefts, widths, 0, 160), 1);
        compare(Work.dropIndex(lefts, widths, 0, 400), 3);
        compare(Work.dropIndex(lefts, widths, 3, 120), 1);
        compare(Work.dropIndex(lefts, widths, 3, -50), 0);
        verify(Work.canDrag({ scrolling: true }));
        verify(!Work.canDrag({ scrolling: false }));
        verify(!Work.canDrag({ scrolling: true, minimised: true }));
        verify(!Work.canDrag({ scrolling: true, floating: true }));
    }

    function test_screenMove() {
        var a = { address: "a" }, b = { address: "b" }, c = { address: "c" }, d = { address: "d" };
        var s = [a, b, c, d];
        // Screen order: the drag itself.
        compare(Work.screenMove(s, s, 0, 2), { from: 0, to: 2 });
        compare(Work.screenMove(s, s, 3, 0), { from: 3, to: 0 });
        compare(Work.screenMove(s, s, 1, 1), null);
        // Another order (d, a, c, b shown): the window lands beside the tab it was dropped by.
        var shown = [d, a, c, b];
        compare(Work.screenMove(shown, s, 3, 1), { from: 1, to: 3 }, "b just after d: after d on screen");
        compare(Work.screenMove(shown, s, 0, 1), { from: 3, to: 1 }, "d just after a: after a on screen");
        compare(Work.screenMove(shown, s, 2, 0), { from: 2, to: 0 }, "c dropped first: first on screen");
        compare(Work.screenMove(shown, s, 1, 3), { from: 0, to: 3 }, "a dropped last: last on screen");
        compare(Work.screenMove(shown, s, 1, 0), null, "a dropped first: already first on screen");
        compare(Work.screenMove(shown, [a, b], 0, 1), null, "a window not on screen");
    }

    function test_close_action() {
        compare(Work.closeAction("ask", false), "ask");
        compare(Work.closeAction(undefined, false), "ask", "unset asks");
        compare(Work.closeAction("minimize", false), "minimise");
        compare(Work.closeAction("close", false), "close");
        compare(Work.closeAction("minimize", true), "close", "a minimised window's X closes it");
        compare(Work.closeAction("ask", true), "close");
        compare(Work.CLOSES.length, 3);
    }

    function test_note_arrived() {
        var a = { appName: "Chat", appIcon: "", summary: "Hi" }, b = { appName: "Mail", appIcon: "", summary: "New" };
        var r = Work.noteArrived(null, a, 1);
        verify(r.fresh, "first one");
        r = Work.noteArrived(r.seen, { appName: "Chat", appIcon: "", summary: "Hi" }, 1);
        verify(!r.fresh, "the same one re-sent");
        r = Work.noteArrived(r.seen, b, 2);
        verify(r.fresh);
        r = Work.noteArrived(r.seen, a, 1);
        verify(!r.fresh, "the newest dismissed: the older one again");
        r = Work.noteArrived(r.seen, a, 2);
        verify(r.fresh, "the same text again, with the count up");
        r = Work.noteArrived(r.seen, null, 0);
        verify(!r.fresh);
        r = Work.noteArrived(r.seen, b, 0);
        verify(!r.fresh, "still remembered after the list emptied");
        r = Work.noteArrived(r.seen, { appName: "Mail", appIcon: "", summary: "" }, 0);
        verify(!r.fresh, "the summary hidden while sharing: the same one");
        r = Work.noteArrived(r.seen, b, 0);
        verify(!r.fresh, "and shown again");
        r = Work.noteArrived(r.seen, { appName: "Chat", appIcon: "", summary: "" }, 1);
        verify(r.fresh, "no summary, but the count rose");
    }

    function test_alerts() {
        var none = {};
        var one = Work.withAlerts(none, ["55d0aa", "0xbeef"], "0xbeef");
        compare(Object.keys(one), ["0x55d0aa"], "the focused window gets none");
        verify(Work.withAlerts(one, ["0x55d0aa"], "") === one, "unchanged: the same object");
        verify(Work.withAlerts(one, ["nope"], "") === one, "bad addresses ignored");
        var two = Work.withAlerts(one, ["0x1"], "");
        compare(Object.keys(two).length, 2);
        compare(Object.keys(one).length, 1, "not changed in place");
        verify(Work.keepAlerts(two, ["0x55d0aa", "0x1"], "") === two);
        compare(Object.keys(Work.keepAlerts(two, ["0x55d0aa", "0x1"], "0x1")), ["0x55d0aa"], "focus clears it");
        compare(Object.keys(Work.keepAlerts(two, ["0x1"], "")), ["0x1"], "a closed window's goes");
    }

    function test_with_note() {
        var n = Work.withNote([], { appName: "Chat", appIcon: "chat", summary: "Hi" }, 5);
        compare(n, [{ appName: "Chat", appIcon: "chat", at: 5 }]);
        var before = n;
        n = Work.withNote(n, { appName: "Mail" }, 6);
        compare(before.length, 1, "the old list isn't changed");
        compare(n[1], { appName: "Mail", appIcon: "", at: 6 });
        var many = [];
        for (var k = 0; k < 80; k++)
            many = Work.withNote(many, { appName: "a" + k }, k);
        compare(many.length, Work.NOTES_MAX, "capped");
        compare(many[many.length - 1].appName, "a79", "newest last");
        compare(many[0].appName, "a30", "oldest dropped");
    }

    function test_minimised_tabs_go_last() {
        var ids = function (l) { return l.map(function (w) { return w.address; }); };
        var list = [
            { address: "0x1", title: "", x: 0, y: 0, minimised: true },
            { address: "0x2", title: "", x: 100, y: 0 },
            { address: "0x3", title: "", x: 200, y: 0, minimised: true },
            { address: "0x4", title: "", x: 300, y: 0 }
        ];
        var ctx = { opened: { "0x1": 0, "0x2": 1, "0x3": 2, "0x4": 3 }, lastOrder: ["0x1", "0x2", "0x3", "0x4"] };
        compare(ids(Work.sortWindows(list, "screen", false, ctx)), ["0x2", "0x4", "0x1", "0x3"], "screen order");
        compare(ids(Work.sortWindows(list, "opened", false, ctx)), ["0x2", "0x4", "0x1", "0x3"], "opening order");
        compare(ids(Work.sortWindows(list, "opened", true, ctx)), ["0x4", "0x2", "0x3", "0x1"], "newest first");
        // Already minimised ones keep their order; the one minimised just now goes last.
        ctx.lastMin = ["0x3"];
        compare(ids(Work.sortWindows(list, "opened", false, ctx)), ["0x2", "0x4", "0x3", "0x1"]);
    }
    function test_strip_with_minimised_icons() {
        var m = { minPill: 28, maxPill: 150, gap: 2, medium: 100, arrow: 16, chip: 30 };
        // Two live tabs and two minimised: the minimised take 28 each, the live share the rest.
        var s = Work.strip(4, 400, m, 0, 2);
        compare(s.mode, "pills");
        compare(s.shown, 4);
        verify(s.focusW >= 100, "the focused tab is still Medium");
        verify(2 * s.pillW + 2 * 28 + 3 * 2 <= 400 + 100, "fits");
        var all = Work.strip(2, 100, m, -1, 2);
        compare(all.mode, "pills", "all minimised: nothing to share");
        // Not everything fits: the old scroll/compact decision.
        compare(Work.strip(9, 120, m, 0, 3).mode, Work.strip(9, 120, m, 0).mode);
    }
    function test_lastLive_is_the_rightmost_tiled_window() {
        var w = [
            { address: "0x1", wsId: 1, x: 0, y: 0 },
            { address: "0x2", wsId: 1, x: 900, y: 0 },
            { address: "0x3", wsId: 1, x: 900, y: 400 },
            { address: "0x4", wsId: 2, x: 5000, y: 0 },
            { address: "0x5", wsId: 1, x: 9000, y: 0, minimised: true },
            { address: "0x6", wsId: 1, x: 9000, y: 0, floating: true }
        ];
        compare(Work.lastLive(w, 1, "0x7"), "0x3");
        compare(Work.lastLive(w, 1, "0x3"), "0x2", "never itself");
        compare(Work.lastLive(w, 3, "0x7"), null);
    }
}
