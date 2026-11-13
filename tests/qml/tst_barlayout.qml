import QtQuick
import QtTest
import "../../shell/logic/barlayout.js" as L

TestCase {
    name: "barlayout"

    readonly property var m: ({ unit: 36, gap: 5, zoneGap: 12, sep: 12 })

    function ids(list) {
        return list.map(function (i) { return i.id; });
    }

    function sample() {
        return L.layout({ shell: { bar: {
            start: [{ id: "system" }, { id: "work" }],
            center: [{ id: "clock", size: "large" }],
            end: [{ id: "notif" }, { id: "separator" }, { id: "status" }],
            options: { stocks: { symbols: ["AAPL"] } }
        } } });
    }

    function test_old_switches_without_bar() {
        var b = L.layout({ shell: { islands: { work: false } }, weather_location: { name: "Oslo" } });
        compare(ids(b.start), ["system"]);
        compare(ids(b.end), ["weather", "clock", "notif", "status"]);
        b = L.layout(undefined);
        compare(ids(b.start), ["system", "work"]);
        compare(ids(b.end), ["clock", "notif", "status"]);
    }

    function test_sizes_and_old_names() {
        var b = L.layout({ shell: { bar: {
            start: [{ id: "work", size: "compact" }, { id: "clock", size: "wide" },
                    { id: "news", size: "normal" }, { id: "stocks", size: "huge" }, { id: "nope" }, null]
        } } });
        compare(b.start.map(function (i) { return i.size; }), ["small", "large", null, null]);
        compare(L.sizeOf(b.start[2]), "large", "news is Large by default");
        compare(L.sizeOf(b.start[3]), "medium");
        compare(L.sizeOf({ id: "keyboard" }), "icon");
        compare([L.UNITS.icon, L.UNITS.small, L.UNITS.medium, L.UNITS.large], [1, 2, 4, 6]);
        compare(L.stepSize("small", 1), "medium");
        compare(L.stepSize("icon", -1), null);
        compare(L.sizeForWidth(36, 36), "icon");
        compare(L.sizeForWidth(100, 36), "small");
        compare(L.sizeForWidth(120, 36), "medium");
        compare(L.sizeForWidth(400, 36), "large");
    }

    function test_move_within_and_across_zones() {
        var b = sample();
        compare(ids(L.move(b, "start", 0, 1).start), ["work", "system"]);
        var m2 = L.move(b, "start", 1, 1);
        compare(ids(m2.start), ["system"]);
        compare(ids(m2.center), ["work", "clock"]);
        compare(ids(L.move(b, "end", 0, -1).center), ["clock", "notif"]);
        compare(L.move(b, "start", 0, -1), null);
        compare(L.move(b, "end", 2, 1), null);
        compare(ids(b.start), ["system", "work"], "the original is untouched");
    }

    function test_add_remove_size() {
        var b = sample();
        compare(ids(L.add(b, "stocks", "center", 0).center), ["stocks", "clock"]);
        compare(L.add(b, "clock", "end"), null, "placed already");
        compare(ids(L.add(b, "separator", "start").start), ["system", "work", "separator"]);
        compare(ids(L.remove(b, "end", 1).end), ["notif", "status"]);
        compare(L.setSize(b, "start", 0, "icon").start[0].size, "icon");
        compare(L.setSize(b, "start", 0, "large").start[0].size, null, "its own default is stored as none");
        compare(L.setSize(b, "start", 0, "huge"), null);
        compare(L.toSetting(L.setSize(b, "start", 1, "small")).start, [{ id: "system" }, { id: "work", size: "small" }]);
        compare(L.toSetting(b).center, [{ id: "clock", size: "large" }]);
    }

    function test_fit_keeps_sizes_when_there_is_room() {
        var b = sample();
        var f = L.fit(b, 1900, m);
        compare(f.hiddenCount, 0);
        compare(f.sizes["start:0"], "large");
        compare(f.sizes["center:0"], "large");
        verify(f.width <= 1900);
    }

    function test_fit_steps_down_biggest_first_never_overflows() {
        var b = sample();
        // Asks: system 216, work 216, clock 216, notif 72, sep 12, status 216.
        for (var w = 2000; w >= 0; w -= 37) {
            var f = L.fit(b, w, m);
            verify(f.width <= w || L.count(b) === f.hiddenCount, "fits " + w + ": " + f.width);
            var p = L.place(b, f, w, m);
            var boxes = Object.keys(p).map(function (k) { return p[k]; }).sort(function (a, c) { return a.x - c.x; });
            for (var i = 0; i < boxes.length; i++) {
                verify(boxes[i].x >= 0 && boxes[i].x + boxes[i].w <= w + 0.5, "inside the bar at " + w);
                if (i > 0)
                    verify(boxes[i].x >= boxes[i - 1].x + boxes[i - 1].w, "no overlap at " + w);
            }
        }
        var f2 = L.fit(b, 800, m);
        compare(f2.hiddenCount, 0, "steps down before leaving anything out");
        // Nothing is left out while anything could still step down.
        var tiny = L.fit(b, 150, m);
        verify(tiny.hiddenCount > 0);
        for (var k in tiny.sizes)
            if (!tiny.hidden[k] && k !== "end:1")
                compare(tiny.sizes[k], "icon", k);
        verify(tiny.hidden["end:1"], "the separator goes first");
        verify(!tiny.hidden["end:2"], "status stays longest");
    }

    function test_centre_moves_aside_rather_than_overlap() {
        var b = { start: [{ id: "system" }, { id: "work" }], center: [{ id: "clock" }], end: [], options: {} };
        var p = L.place(b, L.fit(b, 1400, m), 1400, m);
        compare(p["center:0"].x, Math.round((1400 - 144) / 2), "centred when there's room");
        p = L.place(b, L.fit(b, 700, m), 700, m);
        verify(p["center:0"].x >= p["start:1"].x + p["start:1"].w + m.zoneGap, "moved right of the left zone");
    }

    function test_drop_points_and_drop_edits() {
        var b = sample();
        var f = L.fit(b, 1900, m);
        var p = L.place(b, f, 1900, m);
        var pts = L.dropPoints(b, p, 1900);
        // start: before 0, before 1, after 1; center: before 0, after 0; end: 3 + after.
        compare(pts.length, 3 + 2 + 4);
        var atEnd = L.nearestDrop(pts, 1900);
        compare(atEnd.zone, "end");
        compare(atEnd.index, 3);
        compare(ids(L.dropEdit(b, "start", 0, atEnd).end), ["notif", "separator", "status", "system"]);
        compare(L.dropEdit(b, "start", 0, { zone: "start", index: 1, x: 0 }), null, "dropped where it was");
        compare(ids(L.dropEdit(b, "start", 0, { zone: "start", index: 2, x: 0 }).start), ["work", "system"]);
        var empty = { start: [], center: [], end: [{ id: "clock" }], options: {} };
        var ep = L.dropPoints(empty, L.place(empty, L.fit(empty, 1000, m), 1000, m), 1000);
        compare(L.nearestDrop(ep, 10).zone, "start", "an empty zone takes drops");
        compare(L.nearestDrop(ep, 505).zone, "center");
    }

    function test_order_skips_hidden() {
        var b = sample();
        var f = L.fit(b, 150, m);
        var o = L.order(b, f);
        verify(o.length < L.count(b));
        compare(L.order(b, null).length, 6);
    }

    function test_addable_and_names() {
        var b = sample();
        var names = L.addable(b).map(function (i) { return i.id; });
        verify(names.indexOf("clock") < 0 && names.indexOf("stocks") >= 0 && names.indexOf("separator") >= 0);
        compare(L.name("status"), "Status (network, sound, battery)");
        var o = L.withOptions(b, "news", { feeds: ["https://x.org/rss"] });
        compare(o.options.news.feeds, ["https://x.org/rss"]);
        compare(o.options.stocks.symbols, ["AAPL"]);
    }
}
