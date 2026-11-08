.pragma library

// Minimised windows are tagged `uq_ws_<n>` (n = the workspace they came from) and moved to
// special:minimized; Restore needs n back.
function tagFor(ws) {
    return "uq_ws_" + ws;
}

// Hyprland may list dynamic tags with a trailing "*".
function wsFromTags(tags) {
    if (!tags || typeof tags.length !== "number")
        return null;
    for (var i = 0; i < tags.length; i++) {
        var m = /^uq_ws_(\d+)\*?$/.exec(String(tags[i]));
        if (m)
            return parseInt(m[1], 10);
    }
    return null;
}
