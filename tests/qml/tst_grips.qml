import QtQuick
import QtTest
import "../../shell/logic/grips.js" as G

TestCase {
    name: "grips"

    // A 932 x 1004 screen at 0,0; border 4, 16 between frames.
    readonly property var mon: ({ x: 0, y: 0, width: 932, height: 1004 })

    function test_columns_side_by_side() {
        // Two Half columns: 16..454 and 478..916 (frames 12..458 and 474..920).
        var wins = [{ address: "0xb", x: 478, y: 56, w: 438, h: 940 },
                    { address: "0xa", x: 16, y: 56, w: 438, h: 940 }];
        var g = G.grips(wins, mon, 4, 16);
        compare(g.length, 1);
        compare(g[0].kind, "col");
        compare(g[0].address, "0xa", "the left column is resized");
        compare([g[0].x, g[0].w], [458, 16], "the whole gap between the frames");
        compare([g[0].y, g[0].h], [52, 948], "the height they share, frames included");
        compare(G.dragSize(g[0], 100, 30), { w: 538, h: 940 }, "only the width follows");
        compare(G.dragSize(g[0], -1000, 0).w, G.MIN_W);
    }

    function test_stacked_windows() {
        // One column with two windows, beside a single one.
        var wins = [{ address: "0xa", x: 16, y: 56, w: 500, h: 300 },
                    { address: "0xb", x: 16, y: 380, w: 500, h: 616 },
                    { address: "0xc", x: 540, y: 56, w: 438, h: 940 }];
        var g = G.grips(wins, mon, 4, 16);
        var col = g.filter(function (x) { return x.kind === "col"; });
        var row = g.filter(function (x) { return x.kind === "row"; });
        compare(col.length, 1);
        compare([col[0].y, col[0].h], [52, 948], "beside the whole stack");
        compare(row.length, 1);
        compare(row[0].address, "0xa", "the upper window is resized");
        compare([row[0].x, row[0].w, row[0].y, row[0].h], [12, 508, 360, 16]);
        compare(G.dragSize(row[0], 0, 50), { w: 500, h: 350 });
        compare(G.dragSize(row[0], 0, 5000).h, 300 + 616 - G.MIN_H, "the lower one keeps its minimum");
    }

    function test_no_grip_where_windows_are_apart_or_off_screen() {
        // Far apart: not neighbours.
        compare(G.grips([{ address: "0xa", x: 16, y: 56, w: 300, h: 940 },
                         { address: "0xb", x: 600, y: 56, w: 300, h: 940 }], mon, 4, 16).length, 0);
        // Both columns off to the left: the gap isn't on this screen.
        compare(G.grips([{ address: "0xa", x: -1832, y: 56, w: 900, h: 940 },
                         { address: "0xb", x: -908, y: 56, w: 900, h: 940 }], mon, 4, 16).length, 0);
        // A gap at the screen's edge is clipped to it.
        var g = G.grips([{ address: "0xa", x: -908, y: 56, w: 900, h: 940 },
                         { address: "0xb", x: 16, y: 56, w: 900, h: 940 }], mon, 4, 16);
        compare(g.length, 1);
        compare([g[0].x, g[0].w], [0, 12]);
        compare(G.grips([], mon, 4, 16), []);
        compare(G.grips([{ address: "0xa", x: 0, y: 0, w: 10, h: 10 }], null, 4, 16), []);
    }

    function test_second_screen_coordinates() {
        var m2 = { x: 2000, y: 0, width: 1920, height: 1080 };
        var g = G.grips([{ address: "0xa", x: 2016, y: 60, w: 936, h: 1012 },
                         { address: "0xb", x: 2976, y: 60, w: 928, h: 1012 }], m2, 4, 16);
        compare(g.length, 1);
        compare(g[0].x, 2016 + 936 + 4 - 2000, "in the screen's own coordinates");
    }

    function test_only_the_gap_moves() {
        var wins = [{ address: "0xa", x: 16, y: 56, w: 438, h: 940 },
                    { address: "0xb", x: 478, y: 56, w: 438, h: 940 }];
        var g = G.grips(wins, mon, 4, 16)[0];
        compare(g.other.address, "0xb");
        compare(g.max, 438 + 438 - G.MIN_W, "the right one keeps its minimum");
        // Room 932 - 2 * 4 = 924; a column is its window plus 24 (two borders, two gaps_in).
        var c = G.columnsCall(g, 92, 924, 24);
        compare(c.windows[0].address, "0xa", "the growing column goes first");
        compare(c.windows[0].fraction, 0.5996, "(438 + 92 + 24) / 924");
        compare(c.windows[1], { address: "0xb", fraction: 0.4004 });
        compare(c.pin, { address: "0xa", x: 16 }, "the left edge stays where it was");
        var n = G.columnsCall(g, -185, 924, 24);
        compare(n.windows[0].address, "0xb", "narrower: the right one grows first");
        compare(n.windows[0].fraction + n.windows[1].fraction, 1, "together they keep their width");
        var far = G.columnsCall(g, 1000, 924, 24);
        compare(far.windows[1].fraction, Math.round((G.MIN_W + 24) / 924 * 10000) / 10000, "the other keeps its minimum");
        // A stacked drag only resizes the upper window.
        var st = G.grips([{ address: "0xa", x: 16, y: 56, w: 500, h: 300 },
                          { address: "0xb", x: 16, y: 380, w: 500, h: 616 }], mon, 4, 16)[0];
        compare(G.resizes(st, 0, 40), [{ address: "0xa", w: 500, h: 340 }]);
    }
}
