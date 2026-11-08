import QtQuick
import QtTest
import "../../shell/logic/appmenu.js" as M

TestCase {
    name: "appmenu"

    function e(id, name, cats, extra) {
        var o = { id: id, name: name, icon: "", comment: "", categories: cats, noDisplay: false };
        if (extra)
            for (var k in extra)
                o[k] = extra[k];
        return o;
    }

    function test_category_mapping() {
        compare(M.category(e("a", "A", ["Development", "IDE"])), "Development");
        compare(M.category(e("a", "A", ["Graphics"])), "Graphics");
        compare(M.category(e("a", "A", ["Network", "WebBrowser"])), "Internet");
        compare(M.category(e("a", "A", ["AudioVideo", "Player"])), "Multimedia");
        compare(M.category(e("a", "A", ["Audio"])), "Multimedia");
        compare(M.category(e("a", "A", ["Video"])), "Multimedia");
        compare(M.category(e("a", "A", ["Game", "ArcadeGame"])), "Games");
        compare(M.category(e("a", "A", ["Office"])), "Office");
        compare(M.category(e("a", "A", ["Education"])), "Office");
        compare(M.category(e("a", "A", ["Settings"])), "Settings");
        compare(M.category(e("a", "A", ["System", "TerminalEmulator"])), "System");
        compare(M.category(e("a", "A", ["Utility"])), "Utilities");
        // First freedesktop main category wins, in the entry's own order.
        compare(M.category(e("a", "A", ["Qt", "KDE", "Settings", "System"])), "Settings");
        compare(M.category(e("a", "A", ["System", "Settings"])), "System");
        // Unknown, empty, or missing categories fall to Utilities.
        compare(M.category(e("a", "A", ["Qt", "Bogus"])), "Utilities");
        compare(M.category(e("a", "A", [])), "Utilities");
        compare(M.category({ id: "a", name: "A" }), "Utilities");
        compare(M.category(null), "Utilities");
        // A QML string list arrives as an array-like, not a JS array.
        compare(M.category({ categories: { length: 1, 0: "Game" } }), "Games");
    }

    function test_categories_order_and_empty_hidden() {
        var es = [
            e("t", "Term", ["System"]),
            e("f", "Firefox", ["Network"]),
            e("g", "Gimp", ["Graphics"]),
            e("c", "Chromium", ["Network"]),
            e("h", "Hidden", ["Game"], { noDisplay: true }),
            e("u", "Thing", ["Bogus"])
        ];
        var cs = M.categories(es, 2);
        var keys = cs.map(function (c) { return c.key; });
        compare(keys, ["Favourites", "All apps", "Graphics", "Internet", "System", "Utilities"]);
        compare(cs[0].count, 2);
        compare(cs[1].count, 5);   // hidden entry not counted
        compare(cs[3].count, 2);
        compare(cs[5].count, 1);
        // Favourites and All apps are always there, even with nothing.
        var none = M.categories([], 0);
        compare(none.map(function (c) { return c.key; }), ["Favourites", "All apps"]);
        compare(none[0].count, 0);
        compare(none[1].count, 0);
        compare(M.categories(null).length, 2);
    }

    function test_favourites_order_and_missing() {
        var es = [e("a", "Alpha", []), e("b", "Beta", []), e("c", "Gamma", [])];
        var f = M.favourites(["c", "gone", "a", "c"], es);
        compare(f.map(function (x) { return x.id; }), ["c", "a"]);
        compare(M.favourites([], es).length, 0);
        compare(M.favourites(null, es).length, 0);
        compare(M.favourites(["a"], null).length, 0);
        compare(M.favourites([42, null, "b"], es).map(function (x) { return x.id; }), ["b"]);
    }

    function test_apps_in() {
        var es = [e("z", "zed", ["Network"]), e("a", "Alpha", ["Network"]),
                  e("s", "Sys", ["System"]), e("h", "Hid", ["Network"], { noDisplay: true })];
        compare(M.appsIn("All apps", es, []).map(function (x) { return x.id; }), ["a", "s", "z"]);
        compare(M.appsIn("Internet", es, []).map(function (x) { return x.id; }), ["a", "z"]);
        compare(M.appsIn("Favourites", es, ["s", "z"]).map(function (x) { return x.id; }), ["s", "z"]);
        compare(M.appsIn("Games", es, []).length, 0);
        compare(M.appsIn("nonsense", es, []).length, 0);
    }

    function test_menu_entries_copy_categories() {
        var raw = [{ id: "a", name: "A", genericName: "", comment: "c", icon: "i", keywords: [],
                     noDisplay: false, categories: { length: 2, 0: "Audio", 1: "Player" } }, null];
        var m = M.menuEntries(raw);
        compare(m.length, 1);
        compare(m[0].id, "a");
        compare(m[0].comment, "c");
        compare(m[0].categories, ["Audio", "Player"]);
        compare(M.category(m[0]), "Multimedia");
    }

    function test_power_confirm_double_click_and_timeout() {
        compare(M.CONFIRM_MS, 5000);
        var s = M.initialPower();
        // Instant actions fire on the first click.
        var lockR = M.powerConfirm(s, "lock", 1000);
        verify(lockR.fire);
        compare(lockR.label, "");
        verify(M.powerConfirm(s, "logout", 1000).fire);
        verify(M.powerConfirm(s, "suspend", 1000).fire);

        // Reboot: first click arms, second within 5 s fires.
        var r1 = M.powerConfirm(s, "reboot", 10000);
        verify(!r1.fire);
        compare(r1.label, "Click again to restart");
        var r2 = M.powerConfirm(r1.state, "reboot", 14999);
        verify(r2.fire);
        compare(r2.label, "");
        // Firing clears the pending state.
        var r3 = M.powerConfirm(r2.state, "reboot", 15000);
        verify(!r3.fire);

        // Boundary: exactly 5 s later is too late; it re-arms instead.
        var p1 = M.powerConfirm(s, "poweroff", 20000);
        verify(!p1.fire);
        compare(p1.label, "Click again to shut down");
        var p2 = M.powerConfirm(p1.state, "poweroff", 25000);
        verify(!p2.fire);
        compare(p2.label, "Click again to shut down");
        var p3 = M.powerConfirm(p2.state, "poweroff", 25001);
        verify(p3.fire);

        // A different confirm action re-arms rather than firing the first one.
        var x1 = M.powerConfirm(s, "reboot", 0 + 30000);
        var x2 = M.powerConfirm(x1.state, "poweroff", 30100);
        verify(!x2.fire);
        compare(x2.label, "Click again to shut down");
        // An instant action in between fires and drops the pending confirm.
        var y1 = M.powerConfirm(s, "reboot", 40000);
        var y2 = M.powerConfirm(y1.state, "lock", 40100);
        verify(y2.fire);
        verify(!M.powerConfirm(y2.state, "reboot", 40200).fire);
        // A clock that went backwards doesn't count as a second click.
        var b1 = M.powerConfirm(s, "reboot", 50000);
        verify(!M.powerConfirm(b1.state, "reboot", 49000).fire);
        // Unknown actions never fire and leave nothing pending.
        var u = M.powerConfirm(s, "rm -rf", 1);
        verify(!u.fire);
        compare(u.label, "");
        verify(!M.powerConfirm(null, "bogus", 1).fire);
        // Garbage state is treated as nothing pending.
        verify(!M.powerConfirm({ pending: "reboot", at: "x" }, "reboot", 2).fire);
    }

    function test_avatar_sources() {
        compare(M.avatarSources("/home/me", "me"),
                ["file:///home/me/.face", "file:///var/lib/AccountsService/icons/me"]);
        // Paths are percent-encoded per segment, as Spotlight does for local icons.
        compare(M.avatarSources("/home/a b", "a#b"),
                ["file:///home/a%20b/.face", "file:///var/lib/AccountsService/icons/a%23b"]);
        // A relative or missing HOME and a USER with a slash or dots are skipped.
        compare(M.avatarSources("rel", "me"), ["file:///var/lib/AccountsService/icons/me"]);
        compare(M.avatarSources(undefined, "../x"), []);
        compare(M.avatarSources("/home/me", ".."), ["file:///home/me/.face"]);
        compare(M.avatarSources("", ""), []);
    }

    function test_initial_and_hostname() {
        compare(M.initial("solon"), "S");
        compare(M.initial(""), "?");
        compare(M.initial(undefined), "?");
        compare(M.hostname("box", "other\n"), "box");
        compare(M.hostname(undefined, "  myhost \n"), "myhost");
        compare(M.hostname("", "line1\nline2\n"), "line1");
        compare(M.hostname(undefined, ""), "localhost");
        compare(M.hostname(undefined, undefined), "localhost");
    }
}
