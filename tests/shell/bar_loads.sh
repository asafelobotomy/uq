#!/usr/bin/env bash
# Headless smoke test: Bar's islands and the launcher and OSD routes (global shortcuts and the `uq` IPC
# handler) compile and instantiate (offscreen, no Hyprland, no bridge). Spotlight and the app menu
# are PanelWindows, which need a Wayland backend; smoke-shell.sh covers them.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cat > "$tmp/shell.qml" <<QML
import QtQuick
import Quickshell
import "file:$root/shell/bar"
import "file:$root/shell/launcher"
import "file:$root/shell/osd"
import "file:$root/shell/core"
ShellRoot {
    LauncherRoutes { id: routes }
    OsdRoutes { id: osd }
    FloatingWindow {
        implicitWidth: 800; implicitHeight: 60
        SystemIsland { id: sys }
        NotifIsland { x: 500 }
        WorkIsland { id: work; x: 200 }
        TimeIsland { id: time; x: 350 }
        StatusIsland { id: status; x: 450 }
        Component.onCompleted: console.log("LOADED", sys.width > 0, work.width > 0, time.width > 0, status.width > 0, routes.routed, osd.routed)
    }
    // With no bridge answer yet, roles are null: a bar for every screen, no main display.
    QtObject {
        Component.onCompleted: console.log("BARS", Outputs.roles === null,
            Outputs.barScreens(Quickshell.screens).length === Quickshell.screens.length,
            Quickshell.screens.length > 0, Outputs.primaryScreen(Quickshell.screens) === null)
    }
    Timer { interval: 1500; running: true; onTriggered: Qt.quit() }
}
QML
out=$(env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY QT_QPA_PLATFORM=offscreen timeout 30 qs -p "$tmp/shell.qml" --no-duplicate 2>&1)
echo "$out" | grep -q 'LOADED true true true true true true' || { echo "FAIL: islands or launcher routes did not load"; echo "$out"; exit 1; }
echo "$out" | grep -q 'BARS true true true true' || { echo "FAIL: Outputs with roles null is not a bar per screen"; echo "$out"; exit 1; }
# Fail on QML/JS errors from the shell's own files, not on environment services the machine
# lacks (PipeWire, NetworkManager, UPower, MPRIS, tray, D-Bus log ERROR lines of their own).
# The scanner's `Ignoring unresolvable import ".../file:/..."` warning is harmless: Quickshell's
# scanner joins the `file:` URL to the config dir, while Qt resolves it correctly (the islands
# load); an absolute path without `file:` is rejected by Qt, so `file:` is the right form.
if echo "$out" | grep -E 'TypeError|ReferenceError|is not a type|Cannot assign|qml: .*[Ee]rror|Unable to assign|is not a valid import|module .* is not installed|File not found|Failed to load configuration|Cannot read property|is not a function|Syntax[Ee]rror|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? '; then
    echo "FAIL: QML errors"; echo "$out"; exit 1
fi
[ -n "${SHOW:-}" ] && echo "$out"
echo "bar_loads ok"
