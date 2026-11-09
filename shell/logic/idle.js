.pragma library

// Idle stages (spec §2.7). Pure: Idle.qml owns the IdleMonitors and feeds the session state in.
// Stay awake isn't here: every IdleMonitor sets respectInhibitors, so the bar's IdleInhibitor
// blocks all stages in the compositor.

// Seconds; these match uq-core's IdleTimings defaults.
var DEFAULTS = { dim: 240, lock: 300, screenOff: 360, suspend: 1200, lockedOff: 30 };

// Settings.values.idle key for each stage.
var KEYS = { dim: "dim", lock: "lock", screenOff: "screen_off", suspend: "suspend_battery",
             lockedOff: "locked_screen_off" };

// "dim" is listed for completeness: the dim overlay follows the dim monitor's isIdle through a
// binding in Idle.qml, not through act().
var ACTIONS = { dim: "dim", lock: "lock", screenOff: "dpms-off", lockedOff: "dpms-off",
                suspend: "suspend" };

// A missing key (or anything that isn't a positive number) takes the default; an explicit null
// disables the stage.
function secs(idle, key, d) {
    if (!idle || typeof idle !== "object" || !idle.hasOwnProperty(key) || idle[key] === undefined)
        return d;
    var v = idle[key];
    if (v === null)
        return null;
    return (typeof v === "number" && isFinite(v) && v > 0) ? Math.round(v) : d;
}

// idle: Settings.values.idle. Returns {dim, lock, screenOff, suspend, lockedOff}.
function stages(idle) {
    var out = {};
    for (var s in KEYS)
        out[s] = secs(idle, KEYS[s], DEFAULTS[s]);
    return out;
}

function mediaBlocksLock(idle) {
    return !!(idle && typeof idle === "object" && idle.media_blocks_lock === true);
}

// ctx = {locked, secure, media, mediaBlocksLock, onBattery}; st = stages(...) (defaults when
// omitted). A null stage is never enabled.
function enabled(stage, ctx, st) {
    var t = st || DEFAULTS;
    if (!KEYS.hasOwnProperty(stage) || t[stage] === null || t[stage] === undefined)
        return false;
    var c = ctx || {};
    switch (stage) {
    case "dim":
    case "screenOff":
        return !c.locked && !c.media;
    case "lock":
        return !c.locked && (!c.media || !c.mediaBlocksLock);
    case "suspend":
        // Battery alone, so the timeout counts from the last input; suspendAllowed() guards
        // the moment it fires.
        return !!c.onBattery;
    case "lockedOff":
        return !!c.locked;
    }
    return false;
}

// Checked when the suspend monitor goes idle: suspend only behind a secure lock. If the lock
// isn't secure at that moment, that idle period doesn't suspend.
function suspendAllowed(ctx) {
    return !!(ctx && ctx.locked && ctx.secure);
}

function action(stage) {
    return ACTIONS.hasOwnProperty(stage) ? ACTIONS[stage] : null;
}
