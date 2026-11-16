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
