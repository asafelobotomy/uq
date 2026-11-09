import QtQuick
import QtTest
import "../../shell/logic/idle.js" as I

TestCase {
    name: "idle"

    function ctx(o) {
        var c = { locked: false, secure: false, media: false, mediaBlocksLock: false, onBattery: false };
        for (var k in o)
            c[k] = o[k];
        return c;
    }

    function test_idle_stages_defaults() {
        var all = { dim: 240, lock: 300, screenOff: 360, suspend: 1200, lockedOff: 30 };
        compare(I.stages(undefined), all);
        compare(I.stages(null), all);
        compare(I.stages({}), all);
        // Missing keys take the default; an explicit null disables that stage.
        compare(I.stages({ dim: null, lock: 120 }),
                { dim: null, lock: 120, screenOff: 360, suspend: 1200, lockedOff: 30 });
        compare(I.stages({ dim: null, lock: null, screen_off: null, suspend_battery: null,
                           locked_screen_off: null }),
                { dim: null, lock: null, screenOff: null, suspend: null, lockedOff: null });
        compare(I.stages({ dim: 60, lock: 90, screen_off: 600, suspend_battery: 3600,
                           locked_screen_off: 10 }),
                { dim: 60, lock: 90, screenOff: 600, suspend: 3600, lockedOff: 10 });
        // Garbage falls back to the default rather than a zero timeout.
        compare(I.stages({ dim: "x", lock: -5, screen_off: 0 }), all);
        compare(I.mediaBlocksLock(undefined), false);
        compare(I.mediaBlocksLock({ media_blocks_lock: true }), true);
        compare(I.mediaBlocksLock({ media_blocks_lock: "yes" }), false);
    }

    function test_idle_media_rule() {
        var s = I.stages({});
        // Playing media blocks dim and screen-off either way.
        for (var mbl of [false, true]) {
            compare(I.enabled("dim", ctx({ media: true, mediaBlocksLock: mbl }), s), false);
            compare(I.enabled("screenOff", ctx({ media: true, mediaBlocksLock: mbl }), s), false);
            compare(I.enabled("dim", ctx({ mediaBlocksLock: mbl }), s), true);
            compare(I.enabled("screenOff", ctx({ mediaBlocksLock: mbl }), s), true);
            compare(I.enabled("lock", ctx({ mediaBlocksLock: mbl }), s), true);
        }
        // Lock is blocked by media only with media_blocks_lock.
        compare(I.enabled("lock", ctx({ media: true, mediaBlocksLock: false }), s), true);
        compare(I.enabled("lock", ctx({ media: true, mediaBlocksLock: true }), s), false);
        // None of the three run while locked.
        compare(I.enabled("dim", ctx({ locked: true }), s), false);
        compare(I.enabled("lock", ctx({ locked: true }), s), false);
        compare(I.enabled("screenOff", ctx({ locked: true }), s), false);
    }

    function test_idle_suspend_gate() {
        var s = I.stages({});
        // The suspend monitor runs on battery alone, so it counts from the last input.
        compare(I.enabled("suspend", ctx({}), s), false);
        compare(I.enabled("suspend", ctx({ locked: true, secure: true }), s), false);
        compare(I.enabled("suspend", ctx({ onBattery: true }), s), true);
        compare(I.enabled("suspend", ctx({ onBattery: true, locked: true }), s), true);
        compare(I.enabled("suspend", ctx({ onBattery: true, locked: true, secure: true }), s), true);
        // Media doesn't block it.
        compare(I.enabled("suspend", ctx({ onBattery: true, media: true, mediaBlocksLock: true }), s), true);
        // A null stage is never enabled.
        compare(I.enabled("suspend", ctx({ onBattery: true, locked: true, secure: true }),
                          I.stages({ suspend_battery: null })), false);
    }

    function test_suspend_allowed() {
        // Checked when the suspend monitor fires: only behind a secure lock.
        compare(I.suspendAllowed(ctx({})), false);
        compare(I.suspendAllowed(ctx({ onBattery: true })), false);
        compare(I.suspendAllowed(ctx({ onBattery: true, locked: true })), false);
        compare(I.suspendAllowed(ctx({ onBattery: true, secure: true })), false);
        compare(I.suspendAllowed(ctx({ onBattery: true, locked: true, secure: true })), true);
        compare(I.suspendAllowed(ctx({ locked: true, secure: true })), true);
        compare(I.suspendAllowed(null), false);
    }

    function test_idle_locked_off() {
        var s = I.stages({});
        compare(I.enabled("lockedOff", ctx({ locked: true }), s), true);
        compare(I.enabled("lockedOff", ctx({ locked: true, media: true, mediaBlocksLock: true }), s), true);
        compare(I.enabled("lockedOff", ctx({}), s), false);
        compare(I.enabled("lockedOff", ctx({ locked: true }), I.stages({ locked_screen_off: null })), false);
        // Null stages of every kind are never enabled.
        var none = I.stages({ dim: null, lock: null, screen_off: null });
        compare(I.enabled("dim", ctx({}), none), false);
        compare(I.enabled("lock", ctx({}), none), false);
        compare(I.enabled("screenOff", ctx({}), none), false);
        compare(I.enabled("bogus", ctx({}), s), false);
    }

    function test_idle_action() {
        compare(I.action("dim"), "dim");
        compare(I.action("lock"), "lock");
        compare(I.action("screenOff"), "dpms-off");
        compare(I.action("lockedOff"), "dpms-off");
        compare(I.action("suspend"), "suspend");
        compare(I.action("bogus"), null);
    }
}
