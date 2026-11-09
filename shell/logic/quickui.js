.pragma library

// Pure helpers for the quick-settings window's controls (QuickSettings, AudioGroup, PowerGroup).
// No Quickshell imports; live Pipewire/UPower objects are only read through the plain fields
// named below, so tests can pass plain objects.

// Fixed argv for an absolute brightness. The percent is rounded and clamped to 0..100
// (brightnessctl's -n2 keeps the panel lit at 0). Anything that isn't a finite number -> null.
function brightnessSetArgs(percent) {
    if (typeof percent !== "number" || !isFinite(percent))
        return null;
    var n = Math.max(0, Math.min(100, Math.round(percent)));
    return ["brightnessctl", "-m", "-e4", "-n2", "set", n + "%"];
}

// A slider move from `prev` to `next` (both 0..1). Raising from 0 while muted unmutes; any
// other move leaves mute as it is. A non-finite `next` -> null (no change).
function volumeSet(prev, next, muted) {
    var n = Number(next);
    if (next === null || next === undefined || !isFinite(n))
        return null;
    n = Math.max(0, Math.min(1, n));
    var p = Number(prev);
    var fromZero = !(p > 0);
    return { volume: n, unmute: !!muted && fromZero && n > 0 };
}

// Hardware audio sinks from Pipewire.nodes.values: isSink, not a stream, and with audio.
function sinkNodes(nodes) {
    var out = [];
    if (!nodes)
        return out;
    for (var i = 0; i < nodes.length; i++) {
        var n = nodes[i];
        if (n && n.isSink && !n.isStream && n.audio)
            out.push(n);
    }
    return out;
}

// A node's label: description, then nickname, then name. Untrusted text, returned unchanged
// for UqText (plain text, elided).
function nodeLabel(node) {
    if (!node)
        return "Unknown device";
    var keys = ["description", "nickname", "name"];
    for (var i = 0; i < keys.length; i++) {
        var v = node[keys[i]];
        if (v !== null && v !== undefined && String(v) !== "")
            return String(v);
    }
    return "Unknown device";
}

// PowerProfile values offered by the selector (PowerSaver=0, Balanced=1, Performance=2).
function profileOptions(hasPerformance) {
    return hasPerformance ? [0, 1, 2] : [0, 1];
}

// The battery row shows only for a present laptop battery.
function batteryShown(device) {
    return !!device && !!device.isLaptopBattery && !!device.isPresent;
}
