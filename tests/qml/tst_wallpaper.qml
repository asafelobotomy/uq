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

    // --- span ---

    function twoScreens() {
        return [
            { name: "DP-1", x: 0, y: 0, width: 1920, height: 1080 },
            { name: "DP-2", x: 1920, y: 0, width: 2560, height: 1440 }
        ];
    }

    function test_span_geometry_side_by_side() {
        var s = twoScreens();
        var names = ["DP-1", "DP-2"];
        compare(W.spanGeometry(s, "DP-1", names), { x: 0, y: 0, width: 4480, height: 1440 });
        compare(W.spanGeometry(s, "DP-2", names), { x: -1920, y: 0, width: 4480, height: 1440 });
    }

    function test_span_geometry_stacked_and_negative() {
        // A second screen above and to the left of the first (negative coordinates).
        var s = [
            { name: "A", x: 0, y: 0, width: 1920, height: 1080 },
            { name: "B", x: -1280, y: -1024, width: 1280, height: 1024 }
        ];
        var names = ["A", "B"];
        compare(W.spanGeometry(s, "A", names), { x: -1280, y: -1024, width: 3200, height: 2104 });
        compare(W.spanGeometry(s, "B", names), { x: 0, y: 0, width: 3200, height: 2104 });
        // One screen alone: the box is the screen.
        compare(W.spanGeometry(s, "A", ["A"]), { x: 0, y: 0, width: 1920, height: 1080 });
        // A screen that is not spanning has no box.
        compare(W.spanGeometry(s, "A", ["B"]), null);
        compare(W.spanGeometry(s, "A", []), null);
    }

    function test_span_skips_screens_with_their_own_entry() {
        var s = twoScreens();
        s.push({ name: "DP-3", x: 4480, y: 0, width: 1920, height: 1080 });
        var wp = { outputs: { "*": { image: "/a.png", fit: "span" }, "DP-2": { image: "/b.png", fit: "fill" } } };
        compare(W.spanNames(wp, s), ["DP-1", "DP-3"]);
        // The box covers only the spanning screens, so DP-2's own wallpaper doesn't widen it.
        compare(W.spanGeometry(s, "DP-3", W.spanNames(wp, s)), { x: -4480, y: 0, width: 6400, height: 1080 });
        compare(W.spanGeometry(s, "DP-2", W.spanNames(wp, s)), null);
        // Not spanning: no names.
        compare(W.spanNames({ outputs: { "*": { image: "/a.png", fit: "fill" } } }, s), []);
        compare(W.spanNames({ outputs: { "DP-1": { fit: "span" } } }, s), []);
        compare(W.spanNames(null, s), []);
    }

    function test_clock_index_same_for_all_screens() {
        // 5 minute interval, 4 slides: floor(now / 300 s) mod 4, whoever asks.
        var t = 1700000000000;
        var a = W.clockIndex(t, 300, 4, {});
        compare(a, Math.floor(t / 300000) % 4);
        compare(W.clockIndex(t + 1000, 300, 4, {}), a);
        compare(W.clockIndex(t + 300000, 300, 4, {}), (a + 1) % 4);
        compare(W.clockIndex(t, 300, 0, {}), -1);
    }

    function test_clock_index_skips_failed() {
        var t = 300000 * 6;   // floor(t / 300000) = 6, n = 4: index 2
        compare(W.clockIndex(t, 300, 4, {}), 2);
        compare(W.clockIndex(t, 300, 4, { 2: true }), 3);
        compare(W.clockIndex(t, 300, 4, { 2: true, 3: true }), 0);
        compare(W.clockIndex(t, 300, 4, { 0: true, 1: true, 2: true, 3: true }), -1);
        compare(W.clockIndex(t, 300, 1, { 0: true }), -1);
    }

    function test_ms_to_boundary() {
        compare(W.msToBoundary(300000 * 5 + 1000, 300), 299000);
        compare(W.msToBoundary(300000 * 5, 300), 300000);
    }

    function test_span_fills_by_cropping() {
        compare(W.fillMode("span"), Image.PreserveAspectCrop);
    }
}
