import QtQuick
import QtTest
import "../../shell/logic/lookhome.js" as H

TestCase {
    name: "lookhome"

    function e(id, name) { return { id: id, name: name, icon: "", comment: "", noDisplay: false }; }

    function test_pins_are_five_valid_distinct_ids() {
        compare(H.normalisePins(null).length, 5);
        compare(H.normalisePins(["a.desktop", "a.desktop", "../x.desktop", "a b", "b.desktop", "c.desktop"]),
                ["a.desktop", "", "", "", "b.desktop"]);
    }

    function test_set_pin_moves_and_clears() {
        var p = H.setPin([], 0, "a.desktop");
        p = H.setPin(p, 2, "b.desktop");
        compare(p, ["a.desktop", "", "b.desktop", "", ""]);
        p = H.setPin(p, 4, "a.desktop");
        compare(p, ["", "", "b.desktop", "", "a.desktop"]);
        compare(H.setPin(p, 2, ""), ["", "", "", "", "a.desktop"]);
        compare(H.setPin(p, 5, "c.desktop"), p);
        compare(H.setPin(p, -1, "c.desktop"), p);
        compare(H.setPin(p, 1, "bad id"), p);
    }

    function test_recents_newest_first_capped_no_repeats() {
        var r = [];
        for (var i = 0; i < 12; i++)
            r = H.pushRecent(r, "app" + i + ".desktop");
        compare(r.length, 8);
        compare(r[0], "app11.desktop");
        r = H.pushRecent(r, "app9.desktop");
        compare(r[0], "app9.desktop");
        compare(r.filter(function (x) { return x === "app9.desktop"; }).length, 1);
        compare(H.pushRecent(r, "no/pe"), r);
        compare(H.pushRecent([], "firefox")[0], "firefox");
    }

    function test_recent_entries_skip_uninstalled_and_cap_at_five() {
        var es = [e("a.desktop", "A"), e("b.desktop", "B"), e("c.desktop", "C"), e("d.desktop", "D"),
                  e("e.desktop", "E"), e("f.desktop", "F"), e("g.desktop", "G")];
        var r = ["gone.desktop", "g.desktop", "f.desktop", "e.desktop", "d.desktop", "c.desktop", "b.desktop"];
        var out = H.recentEntries(r, es);
        compare(out.map(function (x) { return x.id; }),
                ["g.desktop", "f.desktop", "e.desktop", "d.desktop", "c.desktop"]);
    }

    function test_squares_keep_ids_of_uninstalled_apps() {
        var sq = H.squares(["a.desktop", "gone.desktop"], [e("a.desktop", "A")]);
        compare(sq.length, 5);
        compare(sq[0].entry.name, "A");
        compare(sq[1].id, "gone.desktop");
        compare(sq[1].entry, null);
        compare(sq[2].id, "");
    }

    function test_save_and_read_back() {
        var back = H.parse(H.serialise(["a.desktop"], ["b.desktop", "b.desktop"]));
        compare(back.pins, ["a.desktop", "", "", "", ""]);
        compare(back.recents, ["b.desktop"]);
        compare(H.parse("not json").pins, ["", "", "", "", ""]);
        compare(H.parse("null").recents, []);
    }

    function test_pick_list_is_by_name() {
        var out = H.pickList([e("b.desktop", "beta"), e("a.desktop", "Alpha"), { id: "x.desktop", name: "Hidden", noDisplay: true }], 8);
        compare(out.map(function (x) { return x.name; }), ["Alpha", "beta"]);
    }
}
