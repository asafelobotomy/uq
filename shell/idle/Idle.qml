import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Services.Mpris
import Quickshell.Services.UPower
import "../core"
import "../session"
import "../logic/idle.js" as I

// Idle stages (spec §2.7): one IdleMonitor per stage. Each respects inhibitors, so Stay awake
// (the bar's IdleInhibitor) blocks them all. Dim 240 s, lock 300 s, screens off 360 s, suspend
// on battery 1200 s once the lock is secure, and screens off 30 s after locking. A null stage is
// disabled. Screens come back on through Hyprland's *_enables_dpms options. Suspend counts from
// the last input (the monitor runs on battery alone) and only fires behind a secure lock.
Scope {
    id: root

    readonly property var idle: Settings.values ? Settings.values.idle : undefined
    readonly property var stages: I.stages(idle)
    readonly property var ctx: ({
        locked: Bridge.locked,
        secure: Session.secure,
        media: Mpris.players.values.some(p => p.isPlaying),
        mediaBlocksLock: I.mediaBlocksLock(idle),
        onBattery: UPower.onBattery
    })

    function on(stage) {
        return I.enabled(stage, ctx, stages);
    }

    function act(stage) {
        switch (I.action(stage)) {
        case "lock":
            Session.lock();
            break;
        case "dpms-off":
            Bridge.call("Dispatch", ["dpms", { on: false }]).catch(function (e) {
                console.log("uq: dpms off failed: " + e);
            });
            break;
        case "suspend":
            if (!I.suspendAllowed(ctx))
                break;
            Bridge.call("Power", ["suspend"]).catch(function (e) {
                console.log("uq: suspend failed: " + e);
            });
            break;
        }
    }

    IdleMonitor {
        id: dimMon
        enabled: root.on("dim")
        timeout: root.stages.dim || 0
        respectInhibitors: true
    }

    IdleMonitor {
        enabled: root.on("lock")
        timeout: root.stages.lock || 0
        respectInhibitors: true
        onIsIdleChanged: if (isIdle) root.act("lock")
    }

    IdleMonitor {
        enabled: root.on("screenOff")
        timeout: root.stages.screenOff || 0
        respectInhibitors: true
        onIsIdleChanged: if (isIdle) root.act("screenOff")
    }

    IdleMonitor {
        enabled: root.on("suspend")
        timeout: root.stages.suspend || 0
        respectInhibitors: true
        onIsIdleChanged: if (isIdle) root.act("suspend")
    }

    // Created when the lock begins, so its timeout counts from the moment of locking.
    IdleMonitor {
        enabled: root.on("lockedOff")
        timeout: root.stages.lockedOff || 0
        respectInhibitors: true
        onIsIdleChanged: if (isIdle) root.act("lockedOff")
    }

    Dim {
        shown: dimMon.enabled && dimMon.isIdle
    }
}
