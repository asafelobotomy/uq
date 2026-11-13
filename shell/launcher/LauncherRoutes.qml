import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland
import "../core"
import "../notif"
import "../session"

// Everything that opens the launchers from outside the bar, routed to Surfaces: the Hyprland
// global shortcuts (base.lua binds `uq:spotlight`, `uq:emoji`, `uq:appmenu`, `uq:quick-settings` and
// `uq:notif-centre`, the last routed to NotifBackend, `uq:lock`, routed to Session,
// `uq:power-menu`, and `uq:edit-bar`, routed to BarLayout's edit mode) and the shell IPC target `uq`. No windows in here, so it also loads headless (tests/shell/bar_loads.sh).
// Shortcuts and IPC pass no screen, so a surface opens on Hyprland.focusedMonitor.
Scope {
    id: root

    // True once all eight shortcuts and the IPC handler exist with the names base.lua and the
    // README use (tests/shell/bar_loads.sh prints it).
    readonly property bool routed: spotlightKey.appid === "uq" && spotlightKey.name === "spotlight"
        && emojiKey.appid === "uq" && emojiKey.name === "emoji"
        && appMenuKey.appid === "uq" && appMenuKey.name === "appmenu"
        && quickSettingsKey.appid === "uq" && quickSettingsKey.name === "quick-settings"
        && notifCentreKey.appid === "uq" && notifCentreKey.name === "notif-centre"
        && lockKey.appid === "uq" && lockKey.name === "lock"
        && powerMenuKey.appid === "uq" && powerMenuKey.name === "power-menu"
        && editBarKey.appid === "uq" && editBarKey.name === "edit-bar"
        && ipc.target === "uq" && typeof ipc.open === "function"
        && typeof ipc.close === "function" && typeof ipc.toggle === "function"
        && typeof ipc.lock === "function" && typeof ipc.lockSecure === "function"
        && typeof ipc.editBar === "function"

    // SUPER+Space: Hyprland sends `pressed` on the key press (and `released` after).
    GlobalShortcut {
        id: spotlightKey
        appid: "uq"
        name: "spotlight"
        description: "Open or close Spotlight"
        onPressed: Surfaces.ipc("toggle", "spotlight")
    }

    // SUPER+.: Spotlight opened at ":", the emoji search (Phase 4 spec §3.2). Already open, its
    // query becomes ":".
    GlobalShortcut {
        id: emojiKey
        appid: "uq"
        name: "emoji"
        description: "Open Spotlight to search for emoji"
        onPressed: Surfaces.openWith("spotlight", ":")
    }

    // Tapping SUPER is a `release = true` bind, which Hyprland fires only on the key release
    // and reports to the client as `released` alone, so this listens for that, not `pressed`.
    GlobalShortcut {
        id: appMenuKey
        appid: "uq"
        name: "appmenu"
        description: "Open or close the app menu"
        onReleased: Surfaces.ipc("toggle", "appmenu")
    }

    // SUPER+A.
    GlobalShortcut {
        id: quickSettingsKey
        appid: "uq"
        name: "quick-settings"
        description: "Open or close quick settings"
        onPressed: Surfaces.ipc("toggle", "quicksettings")
    }

    // SUPER+N: the chosen backend's notification centre (noti-fi's, the built-in one, swaync's
    // panel, dunst's history), on the focused monitor. Ignored while locked.
    GlobalShortcut {
        id: notifCentreKey
        appid: "uq"
        name: "notif-centre"
        description: "Open or close the notification centre"
        onPressed: {
            if (!Bridge.locked)
                NotifBackend.toggleCentre();
        }
    }

    // SUPER+L: lock the session (ignored when already locked).
    GlobalShortcut {
        id: lockKey
        appid: "uq"
        name: "lock"
        description: "Lock the screen"
        onPressed: Session.lock()
    }

    // SUPER+Escape: the power menu on the focused monitor (ignored while locked).
    GlobalShortcut {
        id: powerMenuKey
        appid: "uq"
        name: "power-menu"
        description: "Open or close the power menu"
        onPressed: Surfaces.ipc("toggle", "powermenu")
    }

    // qs ipc -p /usr/share/uq/shell call uq open|close|toggle spotlight|appmenu|quicksettings|powermenu
    // Only these three verbs and four names; all ignored while locked. Nothing here unlocks:
    // `lock` only locks and `lockSecure` (from uq-lock) only marks the lock secure.
    // SUPER+ALT+B (and Settings' "Edit the bar"): edit mode on, on the focused screen; again, off.
    GlobalShortcut {
        id: editBarKey
        appid: "uq"
        name: "edit-bar"
        description: "Edit the bar"
        onPressed: BarLayout.toggleEdit(Hyprland.focusedMonitor ? Hyprland.focusedMonitor.name : "")
    }

    IpcHandler {
        id: ipc
        target: "uq"

        function open(surface: string): string {
            return Surfaces.ipc("open", surface);
        }

        function close(surface: string): string {
            return Surfaces.ipc("close", surface);
        }

        function toggle(surface: string): string {
            return Surfaces.ipc("toggle", surface);
        }

        // qs ipc ... call uq lock: "ok", or "ignored" when already locked.
        function lock(): string {
            return Session.lock();
        }

        // qs ipc ... call uq editBar: edit mode on or off; "on" or "off".
        function editBar(): string {
            BarLayout.toggleEdit(Hyprland.focusedMonitor ? Hyprland.focusedMonitor.name : "");
            return BarLayout.editing ? "on" : "off";
        }

        // uq-lock calls this once WlSessionLock.secure is true. Always "ok".
        function lockSecure(): string {
            return Session.lockSecure();
        }
    }
}
