import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland
import "../core"

// Everything that opens the launchers from outside the bar, routed to Surfaces: the Hyprland
// global shortcuts (base.lua binds `uq:spotlight` and `uq:appmenu`) and the shell IPC target
// `uq`. No windows in here, so it also loads headless (tests/shell/bar_loads.sh). Shortcuts and
// IPC pass no screen, so a surface opens on Hyprland.focusedMonitor.
Scope {
    id: root

    // True once both shortcuts and the IPC handler exist with the names base.lua and the README
    // use (tests/shell/bar_loads.sh prints it).
    readonly property bool routed: spotlightKey.appid === "uq" && spotlightKey.name === "spotlight"
        && appMenuKey.appid === "uq" && appMenuKey.name === "appmenu"
        && ipc.target === "uq" && typeof ipc.open === "function"
        && typeof ipc.close === "function" && typeof ipc.toggle === "function"

    // SUPER+Space: Hyprland sends `pressed` on the key press (and `released` after).
    GlobalShortcut {
        id: spotlightKey
        appid: "uq"
        name: "spotlight"
        description: "Open or close Spotlight"
        onPressed: Surfaces.ipc("toggle", "spotlight")
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

    // qs ipc -p /usr/share/uq/shell call uq open|close|toggle spotlight|appmenu
    // Only these three verbs and two names; all ignored while locked. Nothing here unlocks.
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
    }
}
