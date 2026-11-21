.pragma library

// Pure helpers over Settings.values.shell. An older uqd may send no `shell` at all, so
// undefined, null and missing keys all mean the defaults (every island on, 24-hour clock, UK
// dates, Celsius, built-in provider order).

function islandOn(shell, id) {
    if (!shell || !shell.islands)
        return true;
    return shell.islands[id] !== false;
}

function clockFormat(shell) {
    return (shell && shell.clock_24h === false) ? "h:mm AP" : "HH:mm";
}

// shell.date_format: "us" (month first, weeks from Sunday) or "uk" (the default, and what any
// other value means: day first, weeks from Monday).
function dateFormat(shell) {
    return shell && shell.date_format === "us" ? "us" : "uk";
}

// Qt date patterns for `dateFormat`: short (the bar), long (the bar at Large), full (the
// calendar's heading), and the locale the calendar takes its first weekday from.
function datePatterns(shell) {
    if (dateFormat(shell) === "us")
        return { short: "ddd, MMM d", long: "dddd, MMMM d", full: "dddd, MMMM d, yyyy", locale: "en_US" };
    return { short: "ddd d MMM", long: "dddd d MMMM", full: "dddd d MMMM yyyy", locale: "en_GB" };
}

// shell.temp_unit: "f" (Fahrenheit) or "c" (the default, and what any other value means).
function tempUnit(shell) {
    return shell && shell.temp_unit === "f" ? "f" : "c";
}

// Whether a built-in provider is on (it is unless the saved preferences list it as off).
function providerEnabled(shell, id) {
    var list = shell && Array.isArray(shell.providers) ? shell.providers : [];
    for (var i = 0; i < list.length; i++)
        if (list[i] && list[i].id === id)
            return list[i].enabled !== false;
    return true;
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

// shell.edge_indicators: the screen-edge tabs for the window or workspace beside this one
// (shell/edge/). On unless set to false.
function edgeIndicators(shell) {
    return !(shell && shell.edge_indicators === false);
}

// shell.edge_titles: those tabs also show the window's title. Off (icon only) unless true.
function edgeTitles(shell) {
    return !!(shell && shell.edge_titles === true);
}

// shell.fit_windows: keep floating windows inside their screen (shell/windows/FitWindows.qml).
// On unless set to false.
function fitWindows(shell) {
    return !(shell && shell.fit_windows === false);
}

// shell.edge_style: "peek" (a slice of the window at the edge) or "tab" (the default, and what
// any other value means).
function edgeStyle(shell) {
    return shell && shell.edge_style === "peek" ? "peek" : "tab";
}
