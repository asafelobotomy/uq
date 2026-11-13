import QtQuick
import QtTest
import "../../shell/logic/ticker.js" as T

TestCase {
    name: "ticker"

    function test_price() {
        compare(T.price(354.88), "354.88");
        compare(T.price(12345.6), "12,345.60");
        compare(T.price(-1234.5), "-1,234.50");
        compare(T.price(0.012345), "0.01235");
        compare(T.price(0), "0.00");
        compare(T.price(NaN), "–");
        compare(T.price("1"), "–");
    }

    function test_change_has_a_glyph_not_just_colour() {
        compare(T.pct(1.234), "▲1.23%");
        compare(T.pct(-0.1), "▼0.10%");
        compare(T.pct(0), "■0.00%");
        compare(T.pct(null), "");
        compare(T.direction(2), 1);
        compare(T.direction(-2), -1);
        compare(T.direction(0), 0);
        compare(T.direction(undefined), 0);
    }

    function test_quotes_cleaned() {
        var q = T.quotes({ quotes: [
            { symbol: "AAPL", name: "Apple", price: 10, change: 1, change_pct: 10, currency: "USD",
              day_high: 11, day_low: 9, ok: true },
            { symbol: "", price: 1 },
            null,
            { symbol: "BAD", ok: false },
            { symbol: "WAYTOOLONGSYMBOLXX", price: 1 }
        ] });
        compare(q.length, 2);
        compare(q[0].symbol, "AAPL");
        compare(q[0].changePct, 10);
        verify(q[0].ok);
        verify(!q[1].ok);
        compare(q[1].price, null);
        compare(T.quoteText(q[0]), "AAPL 10.00 ▲10.00%");
        compare(T.quoteText(q[1]), "BAD –");
        compare(T.quotes(null), []);
    }

    function test_headlines_only_web_links() {
        var h = T.headlines({ items: [
            { title: "A", link: "https://a.org/1", source: "A org", published: 100 },
            { title: "B", link: "javascript:alert(1)" },
            { title: "", link: "https://x" },
            { title: "C", link: "http://c.org" }
        ] });
        compare(h.map(function (i) { return i.title; }), ["A", "C"]);
        compare(h[1].published, null);
    }

    function test_age_and_state() {
        var now = 1000000 * 1000;
        compare(T.age(1000000 - 30, now), "now");
        compare(T.age(1000000 - 600, now), "10 min");
        compare(T.age(1000000 - 7200, now), "2 h");
        compare(T.age(1000000 - 172800, now), "2 d");
        compare(T.age(null, now), "");
        compare(T.state(null, now), "Loading…");
        compare(T.state({ status: "ok", fetched_at: 1 }, now), "");
        compare(T.state({ status: "stale", fetched_at: 1000000 - 1200 }, now), "Updating… · 20 min old");
        compare(T.state({ status: "error", fetched_at: 1000000 - 3600 }, now), "Can't update · 1 h old");
        compare(T.state({ status: "error" }, now), "Can't reach the source");
    }

    function test_sysmon_formats() {
        compare(T.percent(45.4), "45%");
        compare(T.percent(undefined), "–");
        compare(T.bytes(512), "512 B");
        compare(T.bytes(3 * 1024 * 1024 * 1024), "3.0 GiB");
        compare(T.temp(54.6), "55°");
        compare(T.temp(-0.2), "0°");
        compare(T.temp(null), "");
    }

    function test_world_clock() {
        var z = T.zones([{ zone: "America/New_York", label: "New York", offset_s: -18000, abbrev: "EST" },
                         { zone: "Bad", offset_s: 999999 }, { zone: 3 }]);
        compare(z.length, 1);
        // 2026-11-13 12:05 UTC
        var t = Date.UTC(2026, 10, 13, 12, 5);
        compare(T.zoneTime(t, -18000, true), "07:05");
        compare(T.zoneTime(t, -18000, false), "7:05 AM");
        compare(T.zoneTime(t, 9 * 3600, false), "9:05 PM");
        compare(T.zoneTime(Date.UTC(2026, 10, 13, 23, 30), 9 * 3600, true), "08:30");
        // Local UTC here (offset 0 minutes): Tokyo is already tomorrow at 23:30 UTC.
        compare(T.dayShift(Date.UTC(2026, 10, 13, 23, 30), 9 * 3600, 0), "+1 day");
        compare(T.dayShift(Date.UTC(2026, 10, 13, 2, 0), -5 * 3600, 0), "−1 day");
        compare(T.dayShift(t, 3600, 0), "");
    }

    function test_marquee_rules() {
        verify(T.moving(true, false, false, false, false));
        verify(!T.moving(false, false, false, false, false), "fits: no motion");
        verify(!T.moving(true, true, false, false, false), "reduced motion");
        verify(!T.moving(true, false, true, false, false), "hovered");
        verify(!T.moving(true, false, false, true, false), "focused");
        verify(!T.moving(true, false, false, false, true), "sharing");
        compare(T.scrollMs(100, 50), 2000);
        compare(T.scrollMs(0, 50), 0);
    }

    function test_symbol_and_feed_checks() {
        compare(T.cleanSymbol(" brk.b "), "BRK.B");
        compare(T.cleanSymbol("^gspc"), "^GSPC");
        compare(T.cleanSymbol("a b"), null);
        compare(T.cleanSymbol("^"), null);
        verify(T.validFeed("https://feeds.bbci.co.uk/news/world/rss.xml"));
        verify(!T.validFeed("http://x.org/rss"));
        verify(!T.validFeed("https://x.org/a b"));
        verify(!T.validFeed("https://"));
        for (var i = 0; i < T.PRESET_FEEDS.length; i++)
            verify(T.validFeed(T.PRESET_FEEDS[i].url), T.PRESET_FEEDS[i].url);
    }
}
