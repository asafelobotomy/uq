import QtQuick
import QtTest
import "../../shell/logic/barlayout.js" as L

TestCase {
    name: "barlayout"

    function ids(list) {
        return list.map(function (i) { return i.id; });
    }

    function sample() {
        return L.layout({ shell: { bar: {
            start: [{ id: "system" }, { id: "work" }],
            center: [{ id: "clock", size: "wide" }],
            end: [{ id: "notif" }, { id: "separator" }, { id: "status" }],
            options: { stocks: { symbols: ["AAPL"] } }
        } } });
    }

    function test_old_switches_without_bar() {
        var b = L.layout({ shell: { islands: { work: false } }, weather_location: { name: "Oslo" } });
        compare(ids(b.start), ["system"]);
        compare(ids(b.center), []);
        compare(ids(b.end), ["weather", "clock", "notif", "status"]);
        b = L.layout(undefined);
        compare(ids(b.start), ["system", "work"]);
        compare(ids(b.end), ["clock", "notif", "status"]);
    }

    function test_cleans_bad_entries() {
        var b = L.layout({ shell: { bar: {
            start: [{ id: "work" }, { id: "nope" }, null, 5, { id: "work" }, { id: "clock", size: "huge" }],
            center: "x",
            end: [{ id: "separator" }, { id: "separator" }]
        } } });
        compare(ids(b.start), ["work", "clock"]);
        compare(b.start[1].size, "normal");
        compare(b.center, []);
        compare(ids(b.end), ["separator", "separator"]);
        compare(b.options, {});
    }

    function test_move_within_and_across_zones() {
        var b = sample();
        var m = L.move(b, "start", 0, 1);
        compare(ids(m.start), ["work", "system"]);
        compare(ids(b.start), ["system", "work"], "the original is untouched");
        m = L.move(b, "start", 1, 1);
        compare(ids(m.start), ["system"]);
        compare(ids(m.center), ["work", "clock"]);
        m = L.move(b, "center", 0, 1);
        compare(ids(m.center), []);
        compare(ids(m.end), ["clock", "notif", "separator", "status"]);
        m = L.move(b, "end", 0, -1);
        compare(ids(m.center), ["clock", "notif"]);
        compare(L.move(b, "start", 0, -1), null);
        compare(L.move(b, "end", 2, 1), null);
        verify(!L.canMove(b, "end", 2, 1));
        verify(L.canMove(b, "end", 2, -1));
        compare(L.move(b, "nope", 0, 1), null);
        compare(L.move(b, "start", 9, 1), null);
    }

    function test_add_remove_size() {
        var b = sample();
        var a = L.add(b, "stocks", "center", 0);
        compare(ids(a.center), ["stocks", "clock"]);
        compare(L.add(b, "clock", "end"), null, "placed already");
        a = L.add(b, "separator", "start");
        compare(ids(a.start), ["system", "work", "separator"]);
        compare(L.add(b, "nope", "start"), null);
        var r = L.remove(b, "end", 1);
        compare(ids(r.end), ["notif", "status"]);
        compare(L.remove(b, "end", 7), null);
        var s = L.setSize(b, "start", 0, "compact");
        compare(s.start[0].size, "compact");
        compare(L.setSize(b, "start", 0, "huge"), null);
        compare(L.find(b, "clock"), { zone: "center", index: 0 });
        compare(L.find(b, "stocks"), null);
        verify(L.placed(b, "status") && !L.placed(b, "news"));
    }

    function test_addable() {
        var b = sample();
        var names = L.addable(b).map(function (i) { return i.id; });
        verify(names.indexOf("clock") < 0);
        verify(names.indexOf("stocks") >= 0);
        verify(names.indexOf("separator") >= 0);
        var full = { start: [], center: [], end: [], options: {} };
        for (var i = 0; i < L.MAX_ITEMS; i++)
            full.start.push({ id: "separator", size: "normal" });
        compare(L.addable(full), []);
        compare(L.add(full, "separator", "end"), null);
    }

    function test_to_setting_and_options() {
        var b = sample();
        var s = L.toSetting(b);
        compare(s.start, [{ id: "system" }, { id: "work" }]);
        compare(s.center, [{ id: "clock", size: "wide" }]);
        compare(s.options.stocks.symbols, ["AAPL"]);
        var o = L.withOptions(b, "news", { feeds: ["https://x.org/rss"] });
        compare(o.options.news.feeds, ["https://x.org/rss"]);
        compare(o.options.stocks.symbols, ["AAPL"]);
        verify(b.options.news === undefined, "the original is untouched");
        verify(L.toSetting({ start: [], center: [], end: [], options: {} }).options === undefined);
    }

    function test_names_and_widths() {
        compare(L.name("stocks"), "Stock ticker");
        compare(L.name("zzz"), "zzz");
        compare(L.textWidth("compact", 10), 120);
        compare(L.textWidth("normal", 10), 200);
        compare(L.textWidth("wide", 10), 320);
        compare(L.textWidth("normal", 0), 220);
    }
}
