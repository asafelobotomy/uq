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
        compare(P.cover(wins, "0xb", room, extra), { from: 940 });
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
}
