import QtQuick
import QtTest
import "../../shell/logic/edges.js" as Edges
import "../../shell/logic/shellprefs.js" as ShellPrefs

TestCase {
    name: "edges"

    // A window as Hyprland's clients reply has it.
    function ipc(addr, ws, x, w, extra) {
        var o = {
            address: addr, "class": "app-" + addr, title: "Title " + addr,
            workspace: { id: ws, name: String(ws) },
            at: [x, 74], size: [w, 992], floating: false, pinned: false, hidden: false,
            focusHistoryID: 5
        };
        for (var k in (extra || {}))
            o[k] = extra[k];
        return o;
    }
    function wins(list) {
        return list.map(function (o) { return Edges.fromIpc(o, ""); });
    }

    // The live scrolling layout from a real session: columns out of view sit off-screen.
    function scrolling() {
        return wins([
            ipc("0xe", 1, 4759, 934, { focusHistoryID: 5 }),
            ipc("0xk", 1, -1877, 1882, { focusHistoryID: 3 }),
            ipc("0xp", 1, -2825, 934, { focusHistoryID: 4 }),
            ipc("0xq", 1, 3811, 934, { focusHistoryID: 2 }),
            ipc("0xb", 1, 1915, 1882, { focusHistoryID: 1 }),
            ipc("0xc", 1, 19, 1882, { focusHistoryID: 0 })
        ]);
    }

    function test_fromIpc() {
        compare(Edges.fromIpc(null, ""), null);
        compare(Edges.fromIpc({ address: "0x1" }, ""), null);
        compare(Edges.fromIpc({ at: [0, 0], size: [1, 1] }, ""), null);
        var w = Edges.fromIpc(ipc("0x1", 2, 10, 20), "Live title");
        compare(w.address, "0x1");
        compare(w.ws, 2);
        compare(w.title, "Live title");
        compare(w.cls, "app-0x1");
        compare(w.history, 5);
        compare(Edges.fromIpc(ipc("0x1", 2, 10, 20), "").title, "Title 0x1");
        // Quickshell gives list-likes, not arrays.
        var o = ipc("0x2", 1, 0, 0);
        o.at = { length: 2, 0: 5, 1: 6 };
        o.size = { length: 2, 0: 7, 1: 8 };
        var l = Edges.fromIpc(o, "");
        compare([l.x, l.y, l.w, l.h], [5, 6, 7, 8]);
        o.at = { length: 1, 0: 5 };
        compare(Edges.fromIpc(o, ""), null);
    }

    function test_scrolling_columns() {
        var ws = scrolling();
        compare(Edges.neighbour(ws, 1, "left", [1], null).address, "0xk");
        compare(Edges.neighbour(ws, 1, "right", [1], null).address, "0xb");
        compare(Edges.neighbour(ws, 1, "left", [1], null).workspace, 0);
        // Stepping: with the reference moved to the window just focused, the next one follows.
        compare(Edges.neighbour(ws, 1, "left", [1], "0xk").address, "0xp");
        compare(Edges.neighbour(ws, 1, "right", [1], "0xb").address, "0xq");
        compare(Edges.neighbour(ws, 1, "right", [1], "0xq").address, "0xe");
        // The last column, with no workspace beyond: nothing.
        compare(Edges.neighbour(ws, 1, "right", [1], "0xe"), null);
        compare(Edges.neighbour(ws, 1, "left", [1], "0xp"), null);
    }

    function test_fullscreen_reference() {
        // A fullscreen window covers the monitor; the neighbours are still the columns beside.
        var ws = wins([
            ipc("0xa", 1, -1900, 1880),
            ipc("0xf", 1, 0, 1920, { focusHistoryID: 0, fullscreen: 2 }),
            ipc("0xb", 1, 1930, 1880)
        ]);
        compare(Edges.neighbour(ws, 1, "left", [1], null).address, "0xa");
        compare(Edges.neighbour(ws, 1, "right", [1], null).address, "0xb");
    }

    function test_dwindle_prefers_overlap() {
        // A is the left half; B top right, C bottom right.
        var ws = wins([
            { address: "0xa", workspace: { id: 1 }, at: [0, 0], size: [960, 1080], focusHistoryID: 3 },
            { address: "0xb", workspace: { id: 1 }, at: [960, 0], size: [960, 540], focusHistoryID: 0 },
            { address: "0xc", workspace: { id: 1 }, at: [960, 540], size: [960, 540], focusHistoryID: 1 }
        ]);
        compare(Edges.neighbour(ws, 1, "left", [1], null).address, "0xa");
        compare(Edges.neighbour(ws, 1, "right", [1], null), null, "C is below, not beside");
        // From A, B and C are equally near and equally far from A's middle: the more recently
        // focused one.
        compare(Edges.neighbour(ws, 1, "right", [1], "0xa").address, "0xb");
    }

    function test_floating_and_hidden_ignored() {
        var ws = wins([
            ipc("0xa", 1, -1000, 900),
            ipc("0xf", 1, -500, 300, { floating: true, focusHistoryID: 0 }),
            ipc("0xt", 1, 0, 1900, { focusHistoryID: 1 }),
            ipc("0xh", 1, 2000, 900, { hidden: true }),
            ipc("0xp", 1, 3000, 900, { pinned: true })
        ]);
        // The floating window is focused: the most recent tiled window is the reference.
        compare(Edges.neighbour(ws, 1, "left", [1], null).address, "0xa");
        compare(Edges.neighbour(ws, 1, "right", [1], null), null);
        // Only floating windows: no layout neighbour.
        var fl = wins([ipc("0xf", 1, 0, 300, { floating: true })]);
        compare(Edges.neighbour(fl, 1, "left", [1], null), null);
    }

    function test_workspace_fallback() {
        var ws = wins([
            ipc("0xa", 1, 0, 1900, { focusHistoryID: 0 }),
            ipc("0xb", 3, 0, 900, { focusHistoryID: 4 }),
            ipc("0xc", 3, 950, 900, { focusHistoryID: 2 }),
            ipc("0xd", 5, 0, 900, { focusHistoryID: 3 }),
            ipc("0xm", -99, 0, 900, { focusHistoryID: 1 }),
            ipc("0xo", 4, 0, 900, { focusHistoryID: 1 })
        ]);
        // Workspace 2 is empty and 4 is on another monitor: the next on this one is 3, and its
        // most recently focused window.
        var r = Edges.neighbour(ws, 1, "right", [1, 2, 3, 5], null);
        compare(r.address, "0xc");
        compare(r.workspace, 3);
        compare(Edges.neighbour(ws, 1, "left", [1, 2, 3, 5], null), null);
        // From 5, left is 3.
        compare(Edges.neighbour(ws, 5, "left", [1, 2, 3, 5], null).workspace, 3);
        // An empty workspace still has neighbours on both sides.
        compare(Edges.neighbour(ws, 2, "left", [1, 2, 3, 5], null).address, "0xa");
        compare(Edges.neighbour(ws, 2, "right", [1, 2, 3, 5], null).workspace, 3);
        // Special workspaces and bad input.
        compare(Edges.neighbour(ws, -99, "left", [1], null), null);
        compare(Edges.neighbour(ws, 1, "up", [1], null), null);
        compare(Edges.neighbour(null, 1, "left", [1], null), null);
        // A step into another workspace makes that workspace the reference's.
        compare(Edges.neighbour(ws, 1, "right", [1, 3, 5], "0xc").address, "0xd");
        compare(Edges.neighbour(ws, 1, "left", [1, 3, 5], "0xc").address, "0xb");
    }

    function test_peek_wanted() {
        var mon = { x: 0, y: 0, width: 1920, height: 1080 };
        // Scrolling, maximized column: the neighbours' frames already show at the edges.
        var max = wins([
            ipc("0xk", 1, -1877, 1882, { focusHistoryID: 3, fullscreen: 1 }),
            ipc("0xc", 1, 19, 1882, { focusHistoryID: 0, fullscreen: 1 }),
            ipc("0xb", 1, 1915, 1882, { focusHistoryID: 1, fullscreen: 1 })
        ]);
        var l = Edges.neighbour(max, 1, "left", [1], null);
        var r = Edges.neighbour(max, 1, "right", [1], null);
        verify(!Edges.peekWanted(max, 1, l, mon, null), "the left frame ends at x=5, in view");
        verify(!Edges.peekWanted(max, 1, r, mon, null), "the right frame starts at x=1915");
        // True fullscreen covers the monitor: both need a slice.
        var full = wins([
            ipc("0xk", 1, -1877, 1882, { focusHistoryID: 3 }),
            { address: "0xc", workspace: { id: 1 }, at: [0, 0], size: [1920, 1080], fullscreen: 2, focusHistoryID: 0 },
            ipc("0xb", 1, 1915, 1882, { focusHistoryID: 1 })
        ]);
        verify(Edges.peekWanted(full, 1, Edges.neighbour(full, 1, "left", [1], null), mon, null));
        verify(Edges.peekWanted(full, 1, Edges.neighbour(full, 1, "right", [1], null), mon, null));
        // Dwindle, maximized: the other tile is hidden behind it.
        var dw = wins([
            { address: "0xa", workspace: { id: 1 }, at: [12, 60], size: [1896, 1008], fullscreen: 1, focusHistoryID: 0 },
            { address: "0xb", workspace: { id: 1 }, at: [966, 60], size: [942, 1008], focusHistoryID: 1 }
        ]);
        verify(Edges.peekWanted(dw, 1, Edges.neighbour(dw, 1, "right", [1], null), mon, null));
        // Not fullscreen: never a standing slice.
        var plain = wins([ipc("0xa", 1, 0, 900, { focusHistoryID: 0 }), ipc("0xb", 1, 3000, 900)]);
        verify(!Edges.peekWanted(plain, 1, Edges.neighbour(plain, 1, "right", [1], null), mon, null));
        // Another workspace's window, from a fullscreen one.
        var other = wins([
            { address: "0xa", workspace: { id: 1 }, at: [0, 0], size: [1920, 1080], fullscreen: 2, focusHistoryID: 0 },
            ipc("0xz", 2, 0, 900, { focusHistoryID: 1 })
        ]);
        verify(Edges.peekWanted(other, 1, Edges.neighbour(other, 1, "right", [1, 2], null), mon, null));
        verify(!Edges.peekWanted(other, 1, null, mon, null));
        verify(!Edges.peekWanted(other, 1, { address: "0xz", workspace: 2 }, null, null));
    }

    function test_free_span() {
        var a = { x: 0, y: 0, width: 1920, height: 1080 };
        var b = { x: 1920, y: 0, width: 1920, height: 1080 };
        compare(Edges.freeSpan([a], a, "left"), { top: 0, bottom: 0 });
        compare(Edges.freeSpan([a, b], a, "right"), null, "shared edge");
        compare(Edges.freeSpan([a, b], a, "left"), { top: 0, bottom: 0 });
        compare(Edges.freeSpan([a, b], b, "left"), null);
        // A shorter screen beside covers the top 600: the rest of the edge is free.
        var c = { x: 1920, y: 0, width: 1000, height: 600 };
        compare(Edges.freeSpan([a, c], a, "right"), { top: 600, bottom: 0 });
        // Covering only the middle: the longer free run wins.
        var d = { x: 1920, y: 200, width: 1000, height: 300 };
        compare(Edges.freeSpan([a, d], a, "right"), { top: 500, bottom: 0 });
        // Not touching (a gap) or only touching at a corner: free.
        var e = { x: 1930, y: 0, width: 100, height: 100 };
        compare(Edges.freeSpan([a, e], a, "right"), { top: 0, bottom: 0 });
        var f = { x: 1920, y: 1080, width: 100, height: 100 };
        compare(Edges.freeSpan([a, f], a, "right"), { top: 0, bottom: 0 });
        compare(Edges.freeSpan([a], null, "left"), null);
    }

    function test_wheel_steps() {
        compare(Edges.wheelSteps(0, 120), { steps: 1, acc: 0 });
        compare(Edges.wheelSteps(0, -120), { steps: 1, acc: 0 });
        compare(Edges.wheelSteps(0, 40), { steps: 0, acc: 40 });
        compare(Edges.wheelSteps(100, 40), { steps: 1, acc: 20 });
        compare(Edges.wheelSteps(0, 250), { steps: 2, acc: 10 });
        compare(Edges.wheelSteps(undefined, NaN), { steps: 0, acc: 0 });
    }

    function test_tab_top() {
        compare(Edges.tabTop(500, 40, 1000), 480);
        compare(Edges.tabTop(5, 40, 1000), 0);
        compare(Edges.tabTop(995, 40, 1000), 960);
    }

    function test_label() {
        var n = { title: "Doc", cls: "editor", workspace: 0 };
        compare(Edges.label(n, false, "left"), "Doc");
        compare(Edges.label(n, true, "left"), "Window on the left");
        compare(Edges.label(n, true, "right"), "Window on the right");
        compare(Edges.label({ title: "", cls: "editor", workspace: 0 }, false, "left"), "editor");
        compare(Edges.label({ title: "Doc", cls: "e", workspace: 3 }, false, "right"), "Doc (workspace 3)");
        compare(Edges.label({ title: "Doc", cls: "e", workspace: 3 }, true, "right"), "Workspace 3");
        compare(Edges.label(null, false, "left"), "");
    }

    function test_setting_default_on() {
        verify(ShellPrefs.edgeIndicators(undefined));
        verify(ShellPrefs.edgeIndicators(null));
        verify(ShellPrefs.edgeIndicators({}));
        verify(ShellPrefs.edgeIndicators({ edge_indicators: true }));
        verify(!ShellPrefs.edgeIndicators({ edge_indicators: false }));
        verify(!ShellPrefs.edgeTitles(undefined));
        verify(!ShellPrefs.edgeTitles({}));
        verify(!ShellPrefs.edgeTitles({ edge_titles: "yes" }));
        verify(ShellPrefs.edgeTitles({ edge_titles: true }));
        compare(ShellPrefs.edgeStyle(undefined), "tab");
        compare(ShellPrefs.edgeStyle({ edge_style: "slice" }), "tab");
        compare(ShellPrefs.edgeStyle({ edge_style: "peek" }), "peek");
    }
}
