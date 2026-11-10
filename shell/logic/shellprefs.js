.pragma library

// Pure helpers over Settings.values.shell. An older uqd may send no `shell` at all, so
// undefined, null and missing keys all mean the defaults (every island on, 24-hour clock,
// built-in provider order).

function islandOn(shell, id) {
    if (!shell || !shell.islands)
        return true;
    return shell.islands[id] !== false;
}

function clockFormat(shell) {
    return (shell && shell.clock_24h === false) ? "h:mm AP" : "HH:mm";
}

function providerOrder(shell) {
    var out = [];
    var list = shell && Array.isArray(shell.providers) ? shell.providers : [];
    for (var i = 0; i < list.length; i++) {
        var p = list[i];
        if (p && typeof p.id === "string" && p.enabled !== false)
            out.push(p.id);
    }
    return out;
}
