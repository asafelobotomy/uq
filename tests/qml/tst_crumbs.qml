import QtQuick
import QtTest
import "../../shell/logic/crumbs.js" as C

TestCase {
    name: "crumbs"

    function scan(cats) {
        return JSON.stringify({ schema_version: 1, command: "scan", data: { categories: cats } });
    }

    function test_parse_sums_what_clean_would_free() {
        var r = C.parse(scan([
            { category: "cache", total: 1000, skipped: false, error: null },
            { category: "trash", total: 9999, skipped: false, error: null },
            { category: "dev-cache", total: 5000, skipped: true, error: null },
            { category: "logs", total: 20, skipped: false, error: null },
            { category: "temp", total: 0, skipped: false, error: null },
            { category: "journal", total: 7, skipped: false, error: "denied" }
        ]));
        compare(r.map(function (x) { return x.name; }), ["cache", "logs"]);
        compare(C.total(r), 1020);
        compare(r[0].label, "App caches");
    }

    function test_parse_rejects_what_is_not_a_scan() {
        compare(C.parse(""), null);
        compare(C.parse("not json"), null);
        compare(C.parse("{}"), null);
        compare(C.parse('{"data": {"categories": 3}}'), null);
        compare(C.parse(scan([])).length, 0);
        compare(C.total(null), 0);
    }

    function test_unknown_category_is_cut_to_length() {
        var r = C.parse(scan([{ category: "x".repeat(80), total: 5 }]));
        compare(r[0].label.length, 40);
    }
}
