import QtQuick
import QtTest
import "../../shell/logic/columns.js" as Columns

TestCase {
    name: "columns"

    function wins() {
        return [
            { address: "0xa", ws: "1", floating: false, history: 1, full: true },
            { address: "0xb", ws: "1", floating: false, history: 0, full: false },
            { address: "0xc", ws: "1", floating: true, history: 2, full: false },
            { address: "0xd", ws: "2", floating: false, history: 3, full: true }
        ];
    }

    function test_summary() {
        var s = Columns.summary(wins(), "1");
        compare(s.count, 2, "floating windows are not columns");
        compare(s.active, "0xb");
        verify(!s.activeFull);
        var t = Columns.summary(wins(), "2");
        compare(t.active, "0xd");
        verify(t.activeFull);
        compare(Columns.summary(wins(), "9").count, 0);
        compare(Columns.summary(null, "1").active, "");
    }

    function test_on_open() {
        var F = function (a) { return { address: a, full: true }; };
        var H = function (a) { return { address: a, full: false }; };
        compare(Columns.onOpen({ count: 0, active: "", activeFull: false }, "0xn"), [F("0xn")], "the first window is Full");
        compare(Columns.onOpen(null, "0xn"), [F("0xn")]);
        compare(Columns.onOpen({ count: 1, active: "0xa", activeFull: true }, "0xn"), [F("0xa"), F("0xn")],
                "opened from a Full column: a screen of its own, the Full one stays Full");
        compare(Columns.onOpen({ count: 2, active: "0xa", activeFull: false }, "0xn"), [H("0xn")],
                "from a Half column: Half beside it");
        compare(Columns.onOpen({ count: 1, active: "0xn", activeFull: true }, "0xn"), [], "already counted");
        compare(Columns.onOpen({ count: 0 }, ""), []);
    }

    function test_left_alone() {
        var before = { "1": { count: 3, active: "0xa" }, "2": { count: 1, active: "0xd" }, "3": { count: 2, active: "0xe" } };
        var after = { "1": { count: 1, active: "0xb" }, "2": { count: 1, active: "0xd" }, "3": { count: 0, active: "" } };
        compare(Columns.leftAlone(before, after), [{ ws: "1", address: "0xb" }]);
        compare(Columns.leftAlone(null, after), []);
    }
}
