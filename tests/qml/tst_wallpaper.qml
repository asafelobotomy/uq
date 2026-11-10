import QtQuick
import QtTest
import "../../shell/logic/wallpaper.js" as W

TestCase {
    name: "wallpaper"

    function test_entry_for_name_then_star() {
        var wp = { outputs: { "DP-1": { image: "/a.png" }, "*": { image: "/b.png" } } };
        compare(W.entryFor(wp, "DP-1").image, "/a.png");
        compare(W.entryFor(wp, "HDMI-A-1").image, "/b.png");
        compare(W.entryFor({ outputs: { "DP-1": {} } }, "eDP-1"), null);
        compare(W.entryFor(undefined, "DP-1"), null);
        compare(W.entryFor({}, "DP-1"), null);
        compare(W.entryFor(null, "DP-1"), null);
    }

    function test_slides_filter_and_order() {
        var fmts = ["jpg", "jpeg", "png", "webp", "jxl", "avif"];
        compare(W.slides(["b.PNG", "a.jpg", "c.txt", "10.png", "2.png"], fmts),
                ["2.png", "10.png", "a.jpg", "b.PNG"]);
        compare(W.slides(["noext", ".png", "x.jxl"], ["png"]), [".png"]);
    }

    function test_fill_modes() {
        compare(W.fillMode("fill"), Image.PreserveAspectCrop);
        compare(W.fillMode("fit"), Image.PreserveAspectFit);
        compare(W.fillMode("center"), Image.Pad);
        compare(W.fillMode("tile"), Image.Tile);
        compare(W.fillMode(undefined), Image.PreserveAspectCrop);
        compare(W.fillMode("bogus"), Image.PreserveAspectCrop);
    }

    function test_fallback_when_unreadable() {
        compare(W.fallbackColor(null, "#1f1f22"), "#1f1f22");
        compare(W.fallbackColor({ color: null }, "#1f1f22"), "#1f1f22");
        compare(W.fallbackColor({}, "#1f1f22"), "#1f1f22");
        compare(W.fallbackColor({ color: "#102030" }, "#1f1f22"), "#102030");
        compare(W.fallbackColor({ color: "red" }, "#1f1f22"), "#1f1f22");
    }

    function test_next_index_wraps() {
        compare(W.nextIndex(0, 3), 1);
        compare(W.nextIndex(2, 3), 0);
        compare(W.nextIndex(0, 1), 0);
        compare(W.nextIndex(0, 0), -1);
    }

    function test_skip_errored_slide() {
        compare(W.nextReadable(0, 4, { 1: true, 2: true }), 3);
        compare(W.nextReadable(3, 4, { 0: true }), 1);
        compare(W.nextReadable(0, 3, { 0: true, 1: true, 2: true }), -1);
        compare(W.nextReadable(0, 0, {}), -1);
        compare(W.nextReadable(0, 3, {}), 1);
    }

    function test_file_url() {
        compare(W.fileUrl("/home/a/b.png"), "file:///home/a/b.png");
        compare(W.fileUrl("/h/a#1?x%y z.png"), "file:///h/a%231%3Fx%25y%20z.png");
    }

    function test_interval_default() {
        compare(W.intervalMs({}), 300000);
        compare(W.intervalMs({ interval: 60 }), 60000);
        compare(W.intervalMs({ interval: -1 }), 300000);
    }
}
