.pragma library

// Display roles (uqd's ListOutputs `roles`): which screen is the main display and which screens
// get a bar. Pure. Both functions return the ShellScreen objects themselves, never wrappers:
// Quickshell's Variants keeps its delegates by object identity, so a changed bar choice or an
// unplugged screen rebuilds only the bars concerned.

function named(screens, name) {
    for (var i = 0; i < screens.length; i++)
        if (screens[i].name === name)
            return screens[i];
    return null;
}

// The screen named by `roles.primary`, else null. uqd sends "" unless a main display is chosen
// and connected, so with none chosen this is null and pop-ups follow the focused screen.
function primaryScreen(screens, roles) {
    var p = roles && roles.primary;
    if (typeof p !== "string" || p === "" || !screens)
        return null;
    return named(screens, p);
}

// The screens that get a bar: every screen when `roles` or `roles.bar` is null (or not a list),
// else those named in `roles.bar`. Never empty while there are screens: a list naming no
// connected screen means all of them.
function barScreens(screens, roles) {
    var all = [];
    if (!screens)
        return all;
    for (var i = 0; i < screens.length; i++)
        all.push(screens[i]);
    var bar = roles && roles.bar;
    if (!Array.isArray(bar))
        return all;
    var chosen = [];
    for (var j = 0; j < all.length; j++)
        if (bar.indexOf(all[j].name) >= 0)
            chosen.push(all[j]);
    return chosen.length > 0 ? chosen : all;
}
