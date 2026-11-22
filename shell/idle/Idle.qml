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
// Each monitor goes through a Rearm, so a changed timeout is applied (see Rearm.qml).
// A sixth monitor tells logind when nobody is at the desktop (IdleHint, through the bridge), at
// the first idle stage that is set (idle.js hintTimeout); inhibitors hold it off like the rest.
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
                console.warn("uq[POWER-001] dpms off failed: " + e);
            });
            break;
        case "suspend":
            if (!I.suspendAllowed(ctx))
                break;
            Bridge.call("Power", ["suspend"]).catch(function (e) {
                console.warn("uq[POWER-001] suspend failed: " + e);
            });
            break;
        }
    }

    Rearm {
        id: dimArm
        want: root.on("dim")
        timeout: root.stages.dim || 0
    }

    Rearm {
        id: lockArm
        want: root.on("lock")
        timeout: root.stages.lock || 0
    }

    Rearm {
        id: screenOffArm
        want: root.on("screenOff")
        timeout: root.stages.screenOff || 0
    }

    Rearm {
        id: suspendArm
        want: root.on("suspend")
        timeout: root.stages.suspend || 0
    }

    Rearm {
        id: lockedOffArm
        want: root.on("lockedOff")
        timeout: root.stages.lockedOff || 0
    }

    IdleMonitor {
        id: dimMon
        enabled: dimArm.armed
        timeout: dimArm.timeout
        respectInhibitors: true
    }

    IdleMonitor {
        enabled: lockArm.armed
        timeout: lockArm.timeout
        respectInhibitors: true
        onIsIdleChanged: if (isIdle) root.act("lock")
    }

    IdleMonitor {
        enabled: screenOffArm.armed
        timeout: screenOffArm.timeout
        respectInhibitors: true
        onIsIdleChanged: if (isIdle) root.act("screenOff")
    }

    IdleMonitor {
        enabled: suspendArm.armed
        timeout: suspendArm.timeout
        respectInhibitors: true
        onIsIdleChanged: if (isIdle) root.act("suspend")
    }

    // Enabled when the lock begins, so its timeout counts from the moment of locking.
    IdleMonitor {
        enabled: lockedOffArm.armed
        timeout: lockedOffArm.timeout
        respectInhibitors: true
        onIsIdleChanged: if (isIdle) root.act("lockedOff")
    }

    Dim {
        shown: dimMon.enabled && dimMon.isIdle
    }

    Rearm {
        id: hintArm
        want: true
        timeout: I.hintTimeout(root.stages)
    }

    IdleMonitor {
        id: hintMon
        enabled: hintArm.armed
        timeout: hintArm.timeout
        respectInhibitors: true
    }

    readonly property bool idleNow: hintMon.enabled && hintMon.isIdle
    function sendIdle() {
        if (Bridge.connected)
            Bridge.call("IdleHint", [root.idleNow]).catch(function () {});
    }
    onIdleNowChanged: sendIdle()
    // A new bridge (the shell or bridge restarted) starts from the current state.
    Connections {
        target: Bridge
        function onConnectedChanged() { root.sendIdle(); }
    }
}
