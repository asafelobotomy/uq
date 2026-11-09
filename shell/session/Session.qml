pragma Singleton
import QtQuick
import Quickshell.Io
import "../core"
import "../logic/lock.js" as L

// The session's lock flow (spec §2.7). Bridge.locked (LockState from hyprland-lock-notify) is the
// truth; this only ever starts the lock (`Lock` starts uq-lock.service) and never unlocks.
// - lock(): SUPER+L, `qs ipc call uq lock`, logind's Lock signal and PrepareForSleep(true).
// - lockSecure(): uq-lock reports WlSessionLock.secure; a pending sleep then gets SleepLockReady
//   so the bridge drops its delay inhibitor.
// - Recovery: while locked, every 3 s, `systemctl --user is-active uq-lock.service`; when it isn't
//   active (the lock client died), start it again.
QtObject {
    id: root

    // --- public state ---
    property bool secure: false         // the lock reported secure since this lock began
    property bool sleepPending: false   // PrepareForSleep(true) waits for the lock to be secure

    // --- internals ---
    property bool starting: false       // a Lock call is in flight
    property bool checkStarted: false
    property int recoveryMs: 3000

    // Returns "ok", or "ignored" when already locked.
    function lock() {
        if (Bridge.locked)
            return "ignored";
        startUnit();
        return "ok";
    }

    // Only marks the lock secure while locked; it can't unlock anything. Always "ok".
    function lockSecure() {
        if (Bridge.locked) {
            secure = true;
            if (sleepPending)
                sleepReady();
        }
        return "ok";
    }

    function startUnit() {
        if (starting)
            return;
        starting = true;
        Bridge.call("Lock").then(function () {
            root.starting = false;
        }).catch(function (e) {
            root.starting = false;
            console.log("uq: Lock failed: " + e);
        });
    }

    function sleepReady() {
        sleepPending = false;
        Bridge.call("SleepLockReady").catch(function (e) {
            console.log("uq: SleepLockReady failed: " + e);
        });
    }

    function check() {
        if (!Bridge.locked || checkProc.running)
            return;
        checkStarted = false;
        checkProc.running = true;
    }

    function checked(unitActive) {
        if (unitActive)
            starting = false;
        if (L.recoveryDecision(Bridge.locked, unitActive, starting) === "start")
            startUnit();
    }

    property Connections bridgeConn: Connections {
        target: Bridge
        function onLockedChanged() {
            if (Bridge.locked) {
                root.check();
            } else {
                root.secure = false;
            }
        }
        function onPrepareForSleep(sleeping) {
            if (!sleeping) {
                root.sleepPending = false;
                return;
            }
            root.sleepPending = true;
            if (root.secure)
                root.sleepReady();
            else
                root.lock();
        }
        function onLockRequested() {
            root.lock();
        }
    }

    property Timer recoveryTimer: Timer {
        interval: root.recoveryMs
        repeat: true
        running: Bridge.locked
        onTriggered: root.check()
    }

    // Fixed argv, never a shell. Exit 0 = active.
    property Process checkProc: Process {
        command: ["systemctl", "--user", "is-active", "uq-lock.service"]
        onStarted: root.checkStarted = true
        onExited: function (code) {
            root.checked(code === 0);
        }
        onRunningChanged: {
            // A failed start (no systemctl) emits only runningChanged: treat the unit as not
            // active, since starting an active unit is harmless and an exposed session isn't.
            if (!running && !root.checkStarted)
                root.checked(false);
        }
    }
}
