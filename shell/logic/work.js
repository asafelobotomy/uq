.pragma library

// Workspace id to pass as `from_ws` for Minimise: only real (positive, integer) workspaces;
// windows on special workspaces cannot be minimised again.
function minimiseFrom(ws) {
    if (!ws || typeof ws.id !== "number" || ws.id !== Math.floor(ws.id) || ws.id <= 0)
        return null;
    return ws.id;
}

// "Move to workspace" targets, as integers.
function wsChoices() {
    var out = [];
    for (var i = 1; i <= 10; i++)
        out.push(i);
    return out;
}

// Normalise a Hyprland address to 0x… and validate it against the bridge's pattern.
function normAddr(a) {
    var s = String(a);
    if (s.indexOf("0x") !== 0)
        s = "0x" + s;
    return /^0x[0-9a-f]{1,16}$/.test(s) ? s : null;
}

// Build the [name, args] pair for Bridge.call("Dispatch", ...) from a captured target
// ({addr, ws}); null when the address is invalid or the action is not allowed.
function buildDispatch(name, target, extra) {
    if (!target)
        return null;
    var addr = normAddr(target.addr);
    if (addr === null)
        return null;
    var args = { address: addr };
    var ex = extra || ({});
    switch (name) {
    case "close":
    case "toggle_float":
    case "pin":
        break;
    case "fullscreen":
        if (ex.mode !== "full" && ex.mode !== "maximized")
            return null;
        args.mode = ex.mode;
        break;
    case "move":
        if (typeof ex.workspace !== "number" || ex.workspace !== Math.floor(ex.workspace)
                || ex.workspace < 1 || ex.workspace > 10)
            return null;
        args.workspace = ex.workspace;
        break;
    case "minimise":
        var from = minimiseFrom({ id: target.ws });
        if (from === null)
            return null;
        args.from_ws = from;
        break;
    default:
        return null;
    }
    return [name, args];
}

// Up to this many open windows show as their own button in the Work island; past it the island
// folds to one icon button until it is clicked open.
var MAX_TABS = 5;

// The Work island's window buttons. windows: [{address, cls, title, workspace (name), minimised,
// monitor}] as the bar reads them from Hyprland (untrusted titles, unchanged here); monitor: this
// bar's monitor name ("" for every monitor); focused: the focused window's address; expanded:
// the folded island was clicked open. Windows on other special workspaces (a scratchpad) are left
// out; minimised ones stay, marked. Returns { tabs: [{address, cls, title, focused, minimised}],
// folded: bool, count } with tabs in Hyprland's order.
function taskView(windows, monitor, focused, expanded) {
    var tabs = [];
    var list = windows || [];
    for (var i = 0; i < list.length; i++) {
        var w = list[i];
        if (!w)
            continue;
        var addr = normAddr(w.address);
        if (addr === null)
            continue;
        if (monitor && w.monitor && w.monitor !== monitor)
            continue;
        var ws = typeof w.workspace === "string" ? w.workspace : "";
        if (ws.indexOf("special:") === 0 && w.minimised !== true)
            continue;
        tabs.push({
            address: addr,
            cls: typeof w.cls === "string" ? w.cls : "",
            title: typeof w.title === "string" ? w.title : "",
            focused: addr === normAddr(focused),
            minimised: w.minimised === true
        });
    }
    return { tabs: tabs, folded: tabs.length > MAX_TABS && expanded !== true, count: tabs.length };
}

// Width each button's title may take: the island's share of the bar split between the buttons,
// never less than a short word.
function titleWidth(barWidth, count) {
    var share = (barWidth > 0 ? barWidth : 1280) * 0.4;
    return Math.max(48, Math.floor(share / Math.max(1, count)));
}
