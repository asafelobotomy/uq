.pragma library

// Holding the shell's window rearranging still while the pointer moves or resizes a window
// (shell/services/WindowDrag.qml). Pure, so tests/qml/tst_mousehold.qml covers it without
// Hyprland.
//
// Hyprland lifts a tiled window out of its layout the moment a Super+drag starts, and drops it
// back in when the button is let go. The other windows reflow around the gap meanwhile, and the
// shell's own handlers (screen pages, column widths, keeping windows on screen) used to take
// that for a window opening, closing or turning floating, and answer it by scrolling the view,
// moving focus about, hiding pages and resizing columns, all under the pointer. The mouse binds
// are therefore rendered with two hooks (crates/uq-core/src/keybinds/hypr.rs, MOUSE_*_EVENT)
// that send these custom events when the button goes down and when it comes up.

var BEGIN = "uq-mouse:begin";
var END = "uq-mouse:end";

// What a Hyprland event means for the hold: "begin", "end", or null for any other event.
// Hyprland sends the event Lua's hl.dsp.event(text) posts as name "custom", data the text.
function kind(name, data) {
    if (name !== "custom")
        return null;
    var d = String(data === undefined || data === null ? "" : data);
    return d === BEGIN ? "begin" : d === END ? "end" : null;
}

// Hyprland un-maximizes a Full window the moment it is lifted for a move (it sends `fullscreen>>0`
// right after the focus event naming it), and re-tiles it as an ordinary column. What was Full
// should come back Full. This follows the events of one hold: `focus` is the last window focused
// (activewindowv2), `full` the one that was un-maximized, "" while none was. Returns the new state.
function note(state, name, data) {
    var s = { focus: state.focus, full: state.full, lifted: state.lifted };
    var d = String(data === undefined || data === null ? "" : data);
    if (name === "activewindowv2" && d !== "")
        s.focus = d;
    else if (name === "fullscreen" && d === "0" && s.full === "")
        s.full = s.focus;
    else if (name === "changefloatingmode" && s.lifted === "") {
        // "address,1": a tiled window made floating, which is how a move lifts it.
        var parts = d.split(",");
        if (parts.length === 2 && parts[1] === "1" && parts[0] !== "")
            s.lifted = parts[0];
    }
    return s;
}

// A fresh state for the start of a hold.
function fresh() {
    return { focus: "", full: "", lifted: "" };
}

// Which screen the pointer is over, and where on it: monitors is Hyprland's monitor objects
// (name, x, y, width, height, scale: pixels, in the layout's logical coordinates once divided by
// scale). Returns {screen, x, y} in that screen's own logical coordinates, or null.
function place(monitors, cx, cy) {
    if (typeof cx !== "number" || typeof cy !== "number" || !isFinite(cx) || !isFinite(cy))
        return null;
    for (var name in monitors) {
        var m = monitors[name];
        if (!m || typeof m.x !== "number" || typeof m.y !== "number")
            continue;
        var sc = m.scale > 0 ? m.scale : 1;
        var w = m.width / sc, h = m.height / sc;
        if (cx >= m.x && cx < m.x + w && cy >= m.y && cy < m.y + h)
            return { screen: name, x: cx - m.x, y: cy - m.y };
    }
    return null;
}

// A window address as Hyprland's events give it (no 0x) in the form the shell's lists use.
function address(a) {
    var s = String(a === undefined || a === null ? "" : a);
    if (s === "")
        return "";
    return s.indexOf("0x") === 0 ? s : "0x" + s;
}
