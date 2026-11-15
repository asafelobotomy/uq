import QtQuick
import QtTest
import "../../shell/logic/pages.js" as P

TestCase {
    name: "pages"

    // The nested test screen: 1888 wide, gaps_out 4, so the columns share 1880; each takes its
    // window's width plus 24 (window gap and borders), and a column at the left edge is at x 16.
    readonly property int room: 1880
    readonly property int extra: 24
    readonly property int edge: 16

    function win(a, x, w) { return { address: a, x: x, y: 60, w: w, h: 936 }; }

    // Full A, a lone Half B, Full C, as Hyprland left them with B focused: A half on screen.
    readonly property var straddle: [win("0xa", -924, 1856), win("0xb", 956, 916), win("0xc", 1896, 1856)]

    function test_pages_by_width() {
        var p = P.pages([win("0xa", 16, 1856), win("0xb", 1896, 916), win("0xc", 2836, 916),
                         win("0xd", 3776, 1856)], room, extra);
        compare(p.length, 3, "Full | Half Half | Full");
        compare(p[1].cols.length, 2);
        compare(p[1].x, 1896);
        compare(p[2].x, 3776);
    }

    function test_resized_pair_shares_a_page() {
        // 60% and 40% of the room: 1128 and 752 with their extras.
        var p = P.pages([win("0xa", 16, 1104), win("0xb", 1144, 728)], room, extra);
        compare(p.length, 1);
        var q = P.pages([win("0xa", 16, 1104), win("0xb", 1144, 1104)], room, extra);
        compare(q.length, 2, "two 60% columns don't fit together");
    }

    function test_stacked_windows_are_one_column() {
        var p = P.pages([win("0xa", 16, 916), { address: "0xb", x: 16, y: 520, w: 916, h: 476 },
                         win("0xc", 956, 916)], room, extra);
        compare(p.length, 1);
        compare(P.pageOf(p, "0xb"), p[0]);
    }

    function test_snap_moves_to_the_page_start() {
        compare(P.snap(straddle, "0xb", room, extra, edge), { address: "0xb", x: 16 },
                "the lone Half starts its page");
        compare(P.snap(straddle, "0xa", room, extra, edge), { address: "0xa", x: 16 });
    }

    function test_snap_leaves_an_aligned_view() {
        var wins = [win("0xa", 16, 916), win("0xb", 956, 916), win("0xc", 1896, 1856)];
        compare(P.snap(wins, "0xb", room, extra, edge), null, "already on its page");
        compare(P.snap([win("0xa", 17, 916)], "0xa", room, extra, edge), null, "within rounding");
        compare(P.snap(wins, "0xz", room, extra, edge), null, "not a tiled window here");
    }

    function test_second_half_of_a_pair_shows_the_pair() {
        // Focusing the second Half of a pair after a Full: Hyprland puts it at the left edge.
        var wins = [win("0xa", -2804, 1856), win("0xb", -924, 916), win("0xc", 16, 916),
                    win("0xd", 956, 1856)];
        compare(P.snap(wins, "0xc", room, extra, edge), { address: "0xb", x: 16 });
    }

    function test_geometry() {
        compare(P.geometry({ x: 0, width: 1888, scale: 1, reserved: [0, 44, 0, 0] },
                           { gapLeft: 4, gapRight: 4, windowGap: 8, border: 8 }),
                { room: 1880, extra: 24, left: 16 });
        compare(P.geometry({ x: 1920, width: 3840, scale: 2 },
                           { gapLeft: 4, gapRight: 4, windowGap: 8, border: 8 }),
                { room: 1912, extra: 24, left: 1936 }, "scaled, on a second screen");
        compare(P.geometry(null, {}), null);
    }

    function test_cover_beside_a_lone_half() {
        // Full A | Half B alone | Full C: B's page leaves 940 empty, where C would show.
        var wins = [win("0xa", 16, 1856), win("0xb", 1896, 916), win("0xc", 2836, 1856)];
        compare(P.cover(wins, "0xb", room, extra), { from: 940, width: 940 });
        compare(P.cover(wins, "0xa", room, extra), null, "a Full page");
        compare(P.cover(wins, "0xz", room, extra), null, "not here");
    }

    function test_no_cover_for_a_pair_or_the_last_page() {
        compare(P.cover([win("0xa", 16, 916), win("0xb", 956, 916), win("0xc", 1896, 1856)],
                        "0xa", room, extra), null, "two Halves fill it");
        compare(P.cover([win("0xa", 16, 1856), win("0xb", 1896, 916)], "0xb", room, extra), null,
                "the strip ends there: nothing to hide");
    }

    function test_hidden_beside_a_lone_half() {
        var wins = [win("0xa", 16, 1856), win("0xb", 1896, 916), win("0xc", 2836, 1856),
                    win("0xd", 4716, 1856)];
        compare(P.hidden(wins, "0xb", room, extra), { "0xc": true }, "on the lone Half's page");
        compare(P.hidden(wins, "0xa", room, extra), { "0xc": true }, "and while sliding past it");
        compare(P.hidden(wins, "0xc", room, extra), {}, "shown on its own page");
        compare(P.hidden([win("0xa", 16, 916), win("0xb", 956, 916), win("0xc", 1896, 1856)],
                         "0xa", room, extra), {}, "a full page hides nothing");
    }

    function test_a_window_made_half_keeps_its_page() {
        // Half A alone, then B (Full, starting its page) made Half: B stays at its page's start
        // rather than joining A.
        var wins = [win("0xa", 16, 916), win("0xb", 956, 916), win("0xc", 1896, 1856)];
        compare(P.pages(wins, room, extra).length, 2, "greedy, B would join A");
        var p = P.pages(wins, room, extra, { "0xa": true, "0xb": true, "0xc": true });
        compare(p.length, 3);
        compare(p[1].x, 956);
        compare(P.snap(wins, "0xb", room, extra, edge, { "0xa": true, "0xb": true }), { address: "0xb", x: 16 });
    }

    function test_a_window_opened_beside_a_half_shares_its_page() {
        // N, new and so not a start, fits beside A.
        var p = P.pages([win("0xa", 16, 916), win("0xn", 956, 916), win("0xc", 1896, 1856)],
                        room, extra, { "0xa": true, "0xc": true });
        compare(p.length, 2);
        compare(P.startsOf(p), { "0xa": true, "0xc": true });
    }

    // A: the page shown; D: the tab dropped on it, two pages along.
    function test_drop_on_a_full_page() {
        var wins = [win("0xa", 16, 1856), win("0xb", 1896, 1856), win("0xd", 3776, 1856)];
        var d = P.dropPlan(wins, room, extra, {}, "0xa", "0xd", "right");
        compare(d.steps, -1, "past B, to right after A");
        compare(d.half, ["0xa", "0xd"], "both Half");
        compare(d.starts, { "0xa": true, "0xd": false });
        compare(d.area, { from: 940, width: 940 });
    }

    function test_drop_beside_a_lone_half() {
        var wins = [win("0xd", 16, 1856), win("0xa", 1896, 916), win("0xb", 2836, 1856)];
        var d = P.dropPlan(wins, room, extra, {}, "0xa", "0xd", "left");
        compare(d.steps, 1, "from before A to right after it");
        compare(d.half, ["0xd"], "the Half stays as it is");
        compare(d.area, { from: 940, width: 940 }, "the empty half, whichever side");
    }

    function test_drop_on_a_pair() {
        var wins = [win("0xl", 16, 916), win("0xr", 956, 916), win("0xd", 1896, 1856)];
        var left = P.dropPlan(wins, room, extra, {}, "0xl", "0xd", "left");
        compare(left.steps, -1, "between L and R");
        compare(left.half, ["0xd", "0xr"]);
        compare(left.starts, { "0xl": true, "0xd": true, "0xr": false }, "L pushed out to the left");
        compare(left.area, { from: 0, width: 940 });
        var right = P.dropPlan(wins, room, extra, {}, "0xl", "0xd", "right");
        compare(right.half, ["0xd", "0xl"]);
        compare(right.starts, { "0xl": true, "0xd": false, "0xr": true }, "R pushed out to the right");
        compare(right.area, { from: 940, width: 940 });
    }

    function test_no_drop() {
        var wins = [win("0xl", 16, 916), win("0xr", 956, 916), win("0xd", 1896, 1856)];
        compare(P.dropPlan(wins, room, extra, {}, "0xl", "0xr", "right"), null, "where it is on its own page");
        compare(P.dropPlan(wins, room, extra, {}, "0xl", "0xz", "left"), null, "not here");
    }

    // Full A | Half B alone | Full C, with B put on the right.
    function test_a_lone_half_on_the_right() {
        var wins = [win("0xa", 16, 1856), win("0xb", 1896, 916), win("0xc", 2836, 1856)];
        var r = { "0xb": true };
        compare(P.snap(wins, "0xb", room, extra, edge, {}, r), { address: "0xb", x: 956 },
                "its right edge on the screen's");
        compare(P.cover(wins, "0xb", room, extra, {}, r), { from: 0, width: 940 }, "the left half");
        compare(P.hidden(wins, "0xb", room, extra, {}, r), { "0xa": true }, "the page before is hidden");
        compare(P.hidden(wins, "0xc", room, extra, {}, r), { "0xa": true }, "and while sliding past");
        compare(P.hidden(wins, "0xa", room, extra, {}, r), {}, "shown on its own page");
        compare(P.rightOf(wins, room, extra, {}, { "0xb": true, "0xa": true }), { "0xb": true },
                "a Full page can't be on the right");
        var firstPage = [win("0xb", 16, 916), win("0xc", 956, 1856)];
        compare(P.snap(firstPage, "0xb", room, extra, edge, {}, r), null, "nor the first page");
    }

    function test_drop_a_lone_half_on_its_own_screen() {
        var wins = [win("0xa", 16, 1856), win("0xb", 1896, 916), win("0xc", 2836, 1856)];
        var d = P.dropPlan(wins, room, extra, {}, "0xb", "0xb", "right", {});
        compare(d.steps, 0, "no column moves");
        compare(d.right, { "0xb": true });
        compare(d.area, { from: 940, width: 940 });
        compare(P.dropPlan(wins, room, extra, {}, "0xb", "0xb", "left", {}), null, "already there");
        var back = P.dropPlan(wins, room, extra, {}, "0xb", "0xb", "left", { "0xb": true });
        compare(back.right, { "0xb": false });
        var firstPage = [win("0xb", 16, 916), win("0xc", 956, 1856)];
        compare(P.dropPlan(firstPage, room, extra, {}, "0xb", "0xb", "right", {}), null,
                "the first page stays on the left");
    }

    function test_drop_swaps_a_pair() {
        var wins = [win("0xl", 16, 916), win("0xr", 956, 916), win("0xc", 1896, 1856)];
        var d = P.dropPlan(wins, room, extra, {}, "0xl", "0xl", "right", {});
        compare(d.steps, 1);
        compare(d.starts, { "0xl": false, "0xr": true }, "R begins the page now");
        compare(d.area, { from: 940, width: 940 });
        compare(P.dropPlan(wins, room, extra, {}, "0xl", "0xr", "left", {}).steps, -1);
        compare(P.dropPlan(wins, room, extra, {}, "0xl", "0xl", "left", {}), null, "where it is");
    }

    function test_drop_beside_a_half_on_the_right() {
        // D (elsewhere) onto B, a Half alone on the right: D fills the left half, before B.
        var wins = [win("0xa", 16, 1856), win("0xb", 1896, 916), win("0xc", 2836, 1856), win("0xd", 4716, 1856)];
        var d = P.dropPlan(wins, room, extra, {}, "0xb", "0xd", "right", { "0xb": true });
        compare(d.steps, -2, "past C and B");
        compare(d.half, ["0xd"]);
        compare(d.starts, { "0xb": false, "0xd": true });
        compare(d.right, { "0xb": false });
        compare(d.area, { from: 0, width: 940 });
    }

    function test_shared_pages_are_framed() {
        var wins = [win("0xl", 16, 916), win("0xr", 956, 916), win("0xc", 1896, 1856), win("0xh", 3776, 916)];
        compare(P.shared(wins, "0xr", room, extra), { from: 0, width: 1880 });
        compare(P.shared(wins, "0xc", room, extra), null, "a Full window on its own");
        compare(P.shared(wins, "0xh", room, extra), null, "a Half on its own");
        var stacked = [win("0xa", 16, 1856), { address: "0xb", x: 16, y: 520, w: 1856, h: 476 }];
        compare(P.shared(stacked, "0xa", room, extra), null, "one column, however many windows");
    }
}
