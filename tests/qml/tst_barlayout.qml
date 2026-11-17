import QtQuick
import QtTest
import "../../shell/logic/barlayout.js" as L

TestCase {
    name: "barlayout"

    readonly property var m: ({ unit: 36, gap: 5 })

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
        compare(L.sizeOf({ id: "security" }), "icon");
        verify(L.known("security"));
        compare([L.UNITS.icon, L.UNITS.small, L.UNITS.medium, L.UNITS.large], [1, 2, 4, 6]);
        compare(L.stepSize("small", 1), { size: "medium", units: null });
        compare(L.stepSize("icon", -1), null);
        compare(L.sizeForWidth(36, 36), { size: "icon", units: null });
        compare(L.sizeForWidth(100, 36), { size: "small", units: null });
        compare(L.sizeForWidth(120, 36), { size: "medium", units: null });
        compare(L.sizeForWidth(230, 36), { size: "large", units: null });
        // Past Large: Custom, one Icon size at a time, up to the most.
        compare(L.sizeForWidth(36 * 7, 36), { size: "custom", units: 7 });
        compare(L.sizeForWidth(36 * 9 + 10, 36), { size: "custom", units: 9 });
        compare(L.sizeForWidth(36 * 40, 36), { size: "custom", units: L.MAX_CUSTOM });
        compare(L.stepSize("large", 1), { size: "custom", units: 7 });
        compare(L.stepSize("custom", 1, 7), { size: "custom", units: 8 });
        compare(L.stepSize("custom", -1, 7), { size: "large", units: null });
        compare(L.stepSize("custom", 1, L.MAX_CUSTOM), null);
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

    function test_text_style() {
        var b = sample();
        compare(L.textOf(b.start[0]), L.TEXT_DEFAULT);
        verify(L.isPlainText(L.textOf(b.start[0])));
        var t = L.setText(b, "start", 1, { bold: true, size: 120 });
        compare(t.start[1].text, { bold: true, size: 120 });
        // Merged: one change keeps the rest.
        var t2 = L.setText(t, "start", 1, { font: L.MONO });
        compare(t2.start[1].text, { font: L.MONO, bold: true, size: 120 });
        // Back to the theme's look: nothing stored.
        compare(L.setText(t2, "start", 1, null).start[1].text, undefined);
        compare(L.setText(t, "start", 1, { bold: false, size: 100 }).start[1].text, undefined);
        // Kept through a size change, a move and the save.
        var s1 = L.setSize(t2, "start", 1, "small");
        compare(s1.start[1].text, { font: L.MONO, bold: true, size: 120 });
        compare(L.setSize(t2, "start", 1, "custom", 8).start[1].text.size, 120);
        compare(L.find(L.move(t2, "start", 1, -1), "work").index, 0);
        compare(L.move(t2, "start", 1, -1).start[0].text.bold, true);
        compare(L.toSetting(s1).start[1], { id: "work", size: "small", text: { font: L.MONO, bold: true, size: 120 } });
        // Separators and bad places have none.
        var sep = L.add(b, "separator", "start");
        compare(L.setText(sep, "start", 2, { bold: true }), null);
        compare(L.setText(b, "start", 9, { bold: true }), null);
        // Read from settings: cleaned.
        var lay = L.layout({ shell: { bar: { start: [
            { id: "clock", text: { size: 134, font: "\u0007", bold: "yes", italic: true } },
            { id: "work", text: { size: 999 } },
            { id: "separator", text: { bold: true } }] } } });
        compare(lay.start[0].text, { italic: true, size: 130 });
        compare(lay.start[1].text, { size: L.TEXT_MAX });
        compare(lay.start[2].text, undefined);
        // Steps and fonts.
        compare(L.stepTextSize(100, 1), 110);
        compare(L.stepTextSize(L.TEXT_MAX, 1), null);
        compare(L.stepTextSize(L.TEXT_MIN, -1), null);
        compare(L.fontFamily(L.textOf({}), "Inter", "Mono"), "Inter");
        compare(L.fontFamily(L.textOf({ text: { font: L.MONO } }), "Inter", "Mono"), "Mono");
        compare(L.fontFamily(L.textOf({ text: { font: "Iosevka" } }), "Inter", "Mono"), "Iosevka");
    }

    function test_custom_size() {
        var b = sample();
        var c = L.setSize(b, "start", 1, "custom", 9);
        compare(c.start[1], { id: "work", size: "custom", units: 9 });
        compare(L.unitsOf(c.start[1]), 9);
        compare(L.tierOf("custom"), "large");
        compare(L.sizeLabel("custom", 9), "Custom (9×)");
        compare(L.toSetting(c).start[1], { id: "work", size: "custom", units: 9 });
        // Keeps its width when chosen again without one; a tier drops it.
        compare(L.setSize(c, "start", 1, "custom").start[1].units, 9);
        compare(L.setSize(b, "start", 1, "custom").start[1].units, L.MIN_CUSTOM);
        compare(L.setSize(c, "start", 1, "small").start[1], { id: "work", size: "small" });
        verify(L.sameSize(c.start[1], { size: "custom", units: 9 }));
        verify(!L.sameSize(c.start[1], { size: "custom", units: 8 }));
        // Read from settings: units cleaned, kept only for Custom.
        var lay = L.layout({ shell: { bar: { start: [
            { id: "work", size: "custom", units: 99 }, { id: "clock", size: "small", units: 9 },
            { id: "separator", size: "custom", units: 8 }] } } });
        compare(lay.start[0].units, L.MAX_CUSTOM);
        compare(lay.start[1], { id: "clock", size: "small" });
        compare(lay.start[2], { id: "separator", size: null });
        // Drawn at its width while there is room (not filling); stepped down an Icon size at a
        // time when the cells run out.
        c = L.setFill(c, "start", 1, false);
        var f = L.fit(c, 1900, m);
        compare(f.sizes["start:1"], "custom");
        compare(f.units["start:1"], 9);
        var pw = L.place(c, f, 1900, m)["start:1"].w;
        verify(Math.abs(pw - 9 * m.unit) <= f.grid.pitch / 2, "about nine Icon sizes wide: " + pw);
        // A bar exactly n cells wide.
        function cols(n) { return n * (m.unit + m.gap) / L.CELLS - m.gap; }
        compare(L.fit(c, cols(f.cols), m).units["start:1"], 9, "exactly enough columns");
        var g = L.fit(c, cols(f.cols - 1), m);
        compare(g.sizes["start:1"], "custom");
        compare(g.units["start:1"], 8);
        var h = L.fit(c, cols(f.cols - (L.colsFor(9, f.grid, m) - L.colsFor(6, f.grid, m))), m);
        compare(h.sizes["start:1"], "large");
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
        var f = L.fit(b, 1400, m);
        var p = L.place(b, f, 1400, m);
        var span = L.colsFor(f.units["center:0"], f.grid, m);
        compare(p["center:0"].x, L.colX(Math.round((f.grid.cols - span) / 2), f.grid), "centred on the grid when there's room");
        verify(Math.abs(p["center:0"].x + p["center:0"].w / 2 - 700) <= f.grid.pitch, "about the middle");
        p = L.place(b, L.fit(b, 700, m), 700, m);
        compare(p["center:0"].x, p["start:1"].x + p["start:1"].w + m.gap, "moved right of the left zone, one gap away");
    }

    // The grid spans every bar exactly, from a small laptop to a 4K screen at 1x, with columns
    // close to the unit (Icon islands stay square); a full bar of islands meets both ends, with
    // the same gap everywhere.
    function test_grid_spans_any_screen() {
        var screens = [1280, 1366, 1440, 1600, 1920, 2560, 3440, 3840];
        var gaps = [0, 5, 8, 16];
        for (var s = 0; s < screens.length; s++)
            for (var q = 0; q < gaps.length; q++) {
                var mm = { unit: 36, gap: gaps[q] };
                var w = screens[s] - 24;
                var gr = L.grid(w, mm);
                compare(L.spanWidth(gr.cols, gr, mm), w, "columns span " + w + " exactly");
                verify(Math.abs(gr.pitch * L.CELLS - mm.gap - mm.unit) <= 1, "an Icon step is CELLS cells at " + w);
                compare(L.colsFor(1, gr, mm), L.CELLS, "an Icon island is CELLS cells");
                verify(Math.abs(L.unitWidth(1, gr, mm) - mm.unit) <= 1, "and stays square at " + w);
                // Open windows fills what Large islands at each end leave.
                var bar = { start: [{ id: "system", size: "large" }, { id: "work" }], center: [],
                            end: [{ id: "status", size: "large" }], options: {} };
                var f = L.fit(bar, w, mm);
                compare(f.hiddenCount, 0);
                var p = L.place(bar, f, w, mm);
                var boxes = Object.keys(p).map(function (k) { return p[k]; }).sort(function (a, c) { return a.x - c.x; });
                compare(boxes[0].x, 0);
                compare(boxes[boxes.length - 1].x + boxes[boxes.length - 1].w, w, "meets the right end at " + w);
                for (var j = 1; j < boxes.length; j++)
                    compare(boxes[j].x - (boxes[j - 1].x + boxes[j - 1].w), mm.gap, "one gap between islands at " + w);
            }
    }

    function test_separator_stands_in_the_gap() {
        var b = sample();
        var f = L.fit(b, 1900, m);
        var p = L.place(b, f, 1900, m);
        compare(p["end:1"].x, p["end:0"].x + p["end:0"].w, "right after notif");
        compare(p["end:2"].x, p["end:1"].x + p["end:1"].w, "status right after it");
        verify(Math.abs(p["end:1"].w - (f.grid.pitch + m.gap)) <= 1, "a cell and two gaps between them: " + p["end:1"].w);
        var edge = { start: [{ id: "separator" }, { id: "system" }], center: [], end: [], options: {} };
        compare(L.place(edge, L.fit(edge, 900, m), 900, m)["start:0"].w, 0, "no gap to stand in at a zone's end");
    }

    // Empty space beside a zone goes to the islands there that fill; the centre stays centred.
    function test_fill() {
        var mm = { unit: 36, gap: 16 }, W = 1896;
        var b = { start: [{ id: "system" }, { id: "work" }], center: [{ id: "clock", size: "large" }],
                  end: [{ id: "status" }], options: {} };
        compare(L.fillOf(b.start[1]), true, "Open windows fills by default");
        compare(L.fillOf(b.start[0]), false);
        var f = L.fit(b, W, mm), p = L.place(b, f, W, mm);
        compare(p["start:1"].x + p["start:1"].w + mm.gap, p["center:0"].x, "work reaches the clock");
        verify(Math.abs(p["center:0"].x + p["center:0"].w / 2 - W / 2) <= f.grid.pitch, "clock still centred");
        verify(p["end:0"].x > p["center:0"].x + p["center:0"].w + mm.gap, "the right side stays empty");
        // The right zone's filler takes its side.
        var b2 = L.setFill(b, "end", 0, true);
        compare(b2.end[0].fill, true);
        var p2 = L.place(b2, L.fit(b2, W, mm), W, mm);
        compare(p2["center:0"].x + p2["center:0"].w + mm.gap, p2["end:0"].x, "status reaches the clock");
        compare(p2["end:0"].x + p2["end:0"].w, W);
        // A centre filler takes both sides when nobody else does.
        var b3 = L.setFill(L.setFill(b, "start", 1, false), "center", 0, true);
        compare(b3.start[1].fill, false, "turning the default off is stored");
        compare(L.setFill(b3, "start", 1, true).start[1].fill, undefined, "the default is not stored");
        var p3 = L.place(b3, L.fit(b3, W, mm), W, mm);
        compare(p3["center:0"].x, p3["start:1"].x + p3["start:1"].w + mm.gap);
        compare(p3["center:0"].x + p3["center:0"].w + mm.gap, p3["end:0"].x);
        // No centre zone: the left filler takes the one gap.
        var b4 = { start: [{ id: "work" }], center: [], end: [{ id: "clock" }], options: {} };
        var p4 = L.place(b4, L.fit(b4, W, mm), W, mm);
        compare(p4["start:0"].x + p4["start:0"].w + mm.gap, p4["end:0"].x);
        compare(L.setFill(b, "start", 9, true), null);
        compare(L.toSetting(b2).end[0], { id: "status", fill: true }, "stored for uqd");
    }

    function test_resize_snaps_to_columns() {
        var mm = { unit: 36, gap: 16 }, gr = L.grid(1896, mm);
        compare(L.sizeForWidth(L.unitWidth(14, gr, mm) + 4, gr, mm), { size: "custom", units: 14 });
        compare(L.sizeForWidth(L.unitWidth(4, gr, mm), gr, mm), { size: "medium", units: null });
    }

    // With wide gaps (16px, as between windows), sizes keep about the width they were given:
    // a 14-unit island about 14 × 36px, not 14 columns and their gaps.
    function test_sizes_keep_their_width_on_a_coarse_grid() {
        var mm = { unit: 36, gap: 16 };
        var screens = [1280, 1920, 2560, 3840];
        for (var s = 0; s < screens.length; s++) {
            var gr = L.grid(screens[s] - 24, mm);
            var units = [1, 2, 4, 6, 7, 10, 14, 24];
            for (var i = 0; i < units.length; i++) {
                var w = L.unitWidth(units[i], gr, mm);
                verify(Math.abs(w - units[i] * mm.unit) <= gr.pitch / 2 + 1, units[i] + " units at " + screens[s] + ": " + w);
            }
        }
        // Your bar at 1920 (system, work 14, clock and weather Large, tray 7, notif, status) fits whole.
        var bar = { start: [{ id: "system" }, { id: "separator" }, { id: "work", size: "custom", units: 14 }],
                    center: [{ id: "clock", size: "large" }, { id: "weather", size: "large" }],
                    end: [{ id: "tray", size: "custom", units: 7 }, { id: "separator" }, { id: "notif", size: "icon" }, { id: "status" }],
                    options: {} };
        var f = L.fit(bar, 1896, mm);
        compare(f.hiddenCount, 0);
        compare(f.units["start:2"], 14, "nothing stepped down");
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
