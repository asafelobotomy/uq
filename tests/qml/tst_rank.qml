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

    // Descriptions and keywords match by word, never by scattered letters.
    function test_secondary_fields_need_a_word() {
        var list = [
            { id: "btop.desktop", name: "btop++", icon: "", comment: "Resource monitor that shows usage and stats for processor" },
            { id: "base.desktop", name: "Base", icon: "", comment: "Create queries and reports", keywords: ["database"] }];
        var r = Rank.searchApps("report", list, {});
        compare(r.length, 1);
        compare(r[0].id, "base.desktop");
        compare(Rank.searchApps("data", list, {})[0].id, "base.desktop");
        compare(Rank.searchApps("rmt", list, {}).length, 0);
        // A name's scattered letters must start at a word.
        var office = [{ id: "impress.desktop", name: "LibreOffice Impress", icon: "", comment: "" }];
        compare(Rank.searchApps("fire", office, {}).length, 0);
        compare(Rank.searchApps("lim", office, {})[0].id, "impress.desktop");
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

    function test_builtin_groups_in_type_order() {
        var g = function (id, name) {
            return { provider: id, provider_name: name, generation: 1, items: [{ id: id, score: 0.5 }] };
        };
        var pg = {
            a: g("files:images", "Images"), b: g("settings", "Settings"), c: g("org.ext", "Ext"),
            d: g("files:folders", "Folders"), e: g("windows", "Windows"), f: g("files:system", "System files"),
            h: g("files:contents", "Contents")
        };
        var m = Rank.merge([{ id: "x", name: "X", score: 0.5, kind: "fuzzy" }], pg, 1);
        compare(m.groups.map(function (x) { return x.provider; }),
                ["apps", "windows", "settings", "files:folders", "files:images", "files:contents", "files:system",
                 "org.ext"]);
    }

    function test_clear_web_address_is_the_top_hit() {
        var apps = [{ id: "gh", name: "GitHub", score: 1.0, kind: "exact" }];
        var strong = { web: { provider: "web", provider_name: "Web", generation: 2,
                              items: [{ id: "https://github.com/", score: 1 }] } };
        compare(Rank.merge(apps, strong, 2).topHit.provider, "web");
        var weak = { web: { provider: "web", provider_name: "Web", generation: 2,
                            items: [{ id: "https://notes.md/", score: 0.45 }] } };
        compare(Rank.merge(apps, weak, 2).topHit.provider, "apps");
    }

    function test_search_windows() {
        var wins = [
            { address: "0x1", title: "Inbox - Mail", cls: "thunderbird", app: "Thunderbird", icon: "tb", workspace: "2" },
            { address: "0x2", title: "~/Projects", cls: "kitty", app: "kitty", icon: "kitty", workspace: "1" },
            { address: "0x3", title: "notes", cls: "kitty", app: "kitty", minimised: true, toWs: 4 },
            { address: "", title: "kitty" }
        ];
        compare(Rank.searchWindows("k", wins).length, 0, "one letter is too short");
        var k = Rank.searchWindows("kitty", wins);
        compare(k.length, 2);
        compare(k[0].subtitle.indexOf("kitty"), 0);
        var mail = Rank.searchWindows("inbox", wins);
        compare(mail.length, 1);
        compare(mail[0].id, "0x1");
        compare(mail[0].title, "Inbox - Mail");
        compare(mail[0].subtitle, "Thunderbird · Workspace 2");
        verify(mail[0].score <= 0.85);
        var min = Rank.searchWindows("notes", wins)[0];
        compare(min.subtitle, "kitty · Minimised");
        verify(min.minimised);
        compare(min.toWs, 4);
        compare(Rank.searchWindows("kitty", wins, 1).length, 1);
    }
}
