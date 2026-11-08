import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland
import "../core"

// The one Spotlight and the one app menu, plus everything that opens them from outside the bar:
// the Hyprland global shortcuts (base.lua binds `uq:spotlight` and `uq:appmenu`) and the shell
// IPC target `uq`. Which surface is open lives in Surfaces (it also closes and wipes both when
// Bridge.locked becomes true); this file only routes to it. Shortcuts and IPC pass no screen, so
// the surface opens on Hyprland.focusedMonitor.
Scope {
    id: root

    // Both surfaces exist (tests/shell/bar_loads.sh checks this headless).
    readonly property bool loaded: spotlight !== null && appMenu !== null

    Spotlight { id: spotlight }
    AppMenu { id: appMenu }

    // SUPER+Space: Hyprland sends `pressed` on the key press (and `released` after).
    GlobalShortcut {
        appid: "uq"
        name: "spotlight"
        description: "Open or close Spotlight"
        onPressed: Surfaces.ipc("toggle", "spotlight")
    }

    // Tapping SUPER is a `release = true` bind, which Hyprland fires only on the key release
    // and reports to the client as `released` alone, so this listens for that, not `pressed`.
    GlobalShortcut {
        appid: "uq"
        name: "appmenu"
        description: "Open or close the app menu"
        onReleased: Surfaces.ipc("toggle", "appmenu")
    }

    // qs ipc -p /usr/share/uq/shell call uq open|close|toggle spotlight|appmenu
    // Only these three verbs and two names; all ignored while locked. Nothing here unlocks.
    IpcHandler {
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
    }
}
