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
