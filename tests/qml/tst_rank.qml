import QtQuick
import QtTest
import "../../shell/logic/rank.js" as Rank

TestCase {
    name: "rank"

    readonly property var entries: [
        { id: "firefox.desktop", name: "Firefox", icon: "firefox", comment: "Browse the web" },
        { id: "files.desktop", name: "Files", icon: "files", comment: "" },
        { id: "gimp.desktop", name: "GIMP", genericName: "Image Editor", icon: "gimp", comment: "" },
        { id: "hidden.desktop", name: "Firewall", noDisplay: true, icon: "x", comment: "" },
        { id: "my-fire.desktop", name: "My Fire Tool", icon: "t", comment: "" }
    ]

    function test_matchScore_tiers() {
        compare(Rank.matchScore("Fire", "fire"), 1.0);
        compare(Rank.matchScore("fi", "Firefox"), 0.9);
        compare(Rank.matchScore("tool", "My Fire Tool"), 0.75);
        var f = Rank.matchScore("frx", "Firefox");
        verify(f > 0 && f <= 0.6);
        compare(Rank.matchScore("zzz", "Firefox"), 0);
        verify(Rank.matchScore("fire", "f-i-r-e x") <= 0.6);
        verify(Rank.matchScore("fox", "Firefox") > Rank.matchScore("frx", "Firefox"));
    }

    function test_exact_prefix_order() {
        var r = Rank.searchApps("fire", entries, {});
        compare(r[0].name, "Firefox");
        compare(r[0].kind, "prefix");
        compare(r[1].name, "My Fire Tool");
        compare(r[1].kind, "fuzzy");
        var e = Rank.searchApps("files", entries, {});
        compare(e[0].kind, "exact");
    }

    function test_launch_count_boosts_but_not_over_prefix() {
        var es = [
            { id: "a", name: "Fabulous Install Reader Editor", icon: "", comment: "" },
            { id: "b", name: "Firefox", icon: "", comment: "" }
        ];
        var r = Rank.searchApps("fire", es, { a: 1000, b: 0 });
        compare(r[0].id, "b");
        compare(r[1].id, "a");
        verify(r[1].score > r[0].score);
        // Boost still orders within a tier.
        var r2 = Rank.searchApps("fi", [
            { id: "x", name: "Fig", icon: "", comment: "" },
            { id: "y", name: "Fin", icon: "", comment: "" }], { y: 5 });
        compare(r2[0].id, "y");
    }

    function test_secondary_fields() {
        var r = Rank.searchApps("image", entries, {});
        compare(r[0].id, "gimp.desktop");
        compare(r[0].kind, "fuzzy");
    }

    function test_nodisplay_skipped() {
        var r = Rank.searchApps("fire", entries, {});
        for (var i = 0; i < r.length; i++)
            verify(r[i].id !== "hidden.desktop");
    }

    function test_empty_query() {
        compare(Rank.searchApps("", entries, {}).length, 0);
        compare(Rank.searchApps("   ", entries, {}).length, 0);
    }

    function test_limit() {
        compare(Rank.searchApps("f", entries, {}, 2).length, 2);
    }

    function sr(gen, provider, name, items) {
        return { generation: gen, provider: provider, provider_name: name, source_dir: "x", items: items };
    }

    function test_provider_cannot_take_app_top_hit() {
        var apps = Rank.searchApps("fire", entries, {});
        var pg = { "ext": sr(3, "ext", "Ext", [{ id: "1", title: "Firefox", score: 0.9 }]) };
        var m = Rank.merge(apps, pg, 3);
        compare(m.topHit.provider, "apps");
        compare(m.topHit.item.name, "Firefox");
    }

    function test_provider_top_hit_when_no_app_match() {
        var apps = Rank.searchApps("zq", entries, {});
        var pg = { "ext": sr(3, "ext", "Ext", [{ id: "1", title: "lo", score: 0.4 }, { id: "2", title: "hi", score: 0.8 }]) };
        var m = Rank.merge(apps, pg, 3);
        compare(m.topHit.provider, "ext");
        compare(m.topHit.item.id, "2");
    }

    function test_merge_ignores_stale_generation() {
        var pg = { "ext": sr(2, "ext", "Ext", [{ id: "1", title: "a", score: 0.8 }]),
                   "calc": sr(3, "calc", "Calculator", [{ id: "c", title: "4", score: 0.9 }]) };
        var m = Rank.merge([], pg, 3);
        compare(m.groups.length, 1);
        compare(m.groups[0].provider, "calc");
        compare(m.topHit.item.id, "c");
    }

    function test_group_order() {
        var apps = Rank.searchApps("fire", entries, {});
        var it = [{ id: "1", title: "t", score: 0.5 }];
        var pg = { "zed": sr(1, "zed", "Zed", it), "run": sr(1, "run", "Run", it),
                   "alpha": sr(1, "alpha", "Alpha", it), "calc": sr(1, "calc", "Calculator", it) };
        var m = Rank.merge(apps, pg, 1);
        var order = m.groups.map(function (g) { return g.provider; });
        compare(order.join(","), "apps,calc,run,alpha,zed");
    }

    function test_merge_empty() {
        var m = Rank.merge([], {}, 1);
        compare(m.groups.length, 0);
        compare(m.topHit, null);
    }

    function test_merge_respects_order() {
        var pg = {
            calc: { provider: "calc", provider_name: "Calculator", generation: 3, items: [{ id: "c", score: 1 }] },
            run: { provider: "run", provider_name: "Run", generation: 3, items: [{ id: "r", score: 1 }] }
        };
        var def = Rank.merge([], pg, 3);
        compare(def.groups[0].provider, "calc");
        var m = Rank.merge([], pg, 3, ["run", "calc"]);
        compare(m.groups[0].provider, "run");
        compare(m.groups[1].provider, "calc");
        // Empty order falls back to the built-in order.
        compare(Rank.merge([], pg, 3, []).groups[0].provider, "calc");
    }
}
