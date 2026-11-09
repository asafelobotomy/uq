pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import "core"
import "logic/lock.js" as L

// uq-lock: a Quickshell config of its own (uq-lock.service), so killing uq-shell never touches the
// lock. It owns the WlSessionLock and the PAM contexts, exposes no IPC, and exits 0 only after a
// successful unlock (the unit doesn't restart exit 0; any other exit or a kill is restarted, and
// Hyprland keeps the session locked meanwhile).
// core/ and logic/ are symlinks to ../shell/core and ../shell/logic: Quickshell refuses imports
// from outside the config directory (`import "../shell/core"` resolves to qs-blackhole).
ShellRoot {
    id: root

    Component.onCompleted: Quickshell.watchFiles = false

    ThemeIo {
        bridged: false
    }

    Auth {
        id: authority
        onUnlocked: {
            lock.locked = false;
            // One event-loop turn so the unlock request reaches the compositor before exiting.
            quitTimer.start();
        }
    }

    Timer {
        id: quitTimer
        interval: 250
        onTriggered: Qt.quit()
    }

    WlSessionLock {
        id: lock
        locked: true
        onSecureChanged: {
            if (secure)
                Quickshell.execDetached(["qs", "ipc", "-p", L.siblingShellDir(Quickshell.shellDir),
                                         "call", "uq", "lockSecure"]);
        }
        surface: LockSurface {
            auth: authority
        }
    }
}
