.pragma library

// Screens in the scrolling layout (shell/windows/ColumnPolicy.qml): a column is Full (the
// screen's width) or Half, and a Full column is a screen of its own. Pure, so
// tests/qml/tst_columns.qml covers it without Hyprland.
//
// The rules, on workspaces that use the scrolling layout, for tiled windows only:
//   - A window opened while the focused column is Full gets a screen of its own: it is made Full
//     too, in the column Hyprland puts it in (right after the focused one, so the columns that
//     were after it move along). The focused column is kept Full, since Hyprland would otherwise
//     give it back its configured width.
//   - A window opened from a Half column is made Half beside it. (Hyprland would make it Full
//     whenever another column on the workspace is: a Full column is a maximized one there, and
//     new windows take that over.)
//   - The first window on a workspace, and the last one left when others close, minimise or
//     move away, is made Full, so it stays Full when the next window opens.

// A workspace's tiled windows from a snapshot: windows [{address, ws, floating, history, full}]
// where ws is the workspace name and history Hyprland's focusHistoryID (lower = more recent).
// Returns {count, active (address of the most recently focused, or ""), activeFull}.
function summary(wins, ws) {
    var count = 0, best = null;
    for (var i = 0; i < (wins ? wins.length : 0); i++) {
        var w = wins[i];
        if (!w || w.ws !== ws || w.floating === true)
            continue;
        count++;
        if (best === null || w.history < best.history)
            best = w;
    }
    return { count: count, active: best ? best.address : "", activeFull: !!best && best.full === true };
}

// What to do once window `opened` (its address) has opened on a scrolling workspace, given the
// workspace's summary from just before. Returns [{address, full}] to set, in order.
function onOpen(before, opened) {
    if (!opened)
        return [];
    if (!before || before.count === 0)
        return [{ address: opened, full: true }];
    if (before.active === "" || before.active === opened)
        return [];
    if (before.activeFull)
        return [{ address: before.active, full: true }, { address: opened, full: true }];
    return [{ address: opened, full: false }];
}

// After windows closed or moved: the workspaces (names) that went from several tiled windows to
// one, with that window's address. before/after: {ws: summary}. Returns [{ws, address}].
function leftAlone(before, after) {
    var out = [];
    for (var ws in after) {
        var a = after[ws], b = before ? before[ws] : null;
        if (a && a.count === 1 && b && b.count > 1 && a.active !== "")
            out.push({ ws: ws, address: a.active });
    }
    return out;
}
