import QtQuick
import QtTest
import "../../shell/logic/clip.js" as C

TestCase {
    name: "clip"

    function test_parse_list_keeps_good_entries_only() {
        var r = C.parseList(JSON.stringify([
            { id: 2, kind: "text", bytes: 3, created: 5, pinned: true, preview: "abc" },
            { id: "x", kind: "text", preview: "bad id" },
            { id: 3, kind: "video", preview: "bad kind" },
            { id: 4, kind: "image", bytes: 10, created: 1, pinned: false, preview: "", path: "/a/4" }
        ]));
        compare(r.map(function (e) { return e.id; }), [2, 4]);
        compare(r[0].pinned, true);
        compare(r[0].path, "");
        compare(r[1].path, "/a/4");
    }

    function test_parse_list_refuses_what_is_not_a_list() {
        compare(C.parseList("nope"), null);
        compare(C.parseList("{\"ok\":true}"), null);
    }

    function test_parse_state() {
        var s = C.parseState("{\"event\":\"state\",\"paused\":true,\"enabled\":true,\"count\":4}");
        compare(s.paused, true);
        compare(s.count, 4);
        compare(C.parseState("{\"ok\":true}"), null);
        compare(C.parseState("x"), null);
    }

    function test_filter_is_case_insensitive() {
        var es = [{ preview: "Hello World" }, { preview: "other" }];
        compare(C.filter(es, "WORLD").length, 1);
        compare(C.filter(es, "  ").length, 2);
        compare(C.filter(es, "zzz").length, 0);
    }

    function test_label_collapses_whitespace_and_cuts() {
        compare(C.label({ kind: "text", preview: "a\n  b\tc" }), "a b c");
        compare(C.label({ kind: "text", preview: "abcdefgh" }, 5), "abcd…");
        compare(C.label({ kind: "image", preview: "", bytes: 2048 }), "Image  2 KB");
    }

    function test_age() {
        compare(C.age(1000, 1000), "now");
        compare(C.age(0, 5 * 60), "5m");
        compare(C.age(0, 3 * 3600), "3h");
        compare(C.age(0, 2 * 86400), "2d");
        compare(C.age(10, 5), "now");
    }

    function test_retry_backs_off() {
        compare(C.retryDelayMs(0), 5000);
        compare(C.retryDelayMs(1), 10000);
        compare(C.retryDelayMs(2), 30000);
        compare(C.retryDelayMs(3), 60000);
        compare(C.retryDelayMs(99), 60000);
        compare(C.retryDelayMs(-1), 5000);
    }

    function test_move_stays_inside() {
        compare(C.move(0, -1, 3), 0);
        compare(C.move(2, 1, 3), 2);
        compare(C.move(0, 8, 3), 2);
        compare(C.move(0, 1, 0), -1);
    }

    function test_summary() {
        compare(C.summary({ enabled: true, paused: false, count: 1 }), "1 item in the clipboard history");
        compare(C.summary({ enabled: true, paused: true, count: 4 }), "Clipboard history is paused");
        compare(C.summary({ enabled: false, paused: false, count: 4 }), "Clipboard history is off");
    }
}
