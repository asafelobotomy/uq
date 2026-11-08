#!/usr/bin/env bash
# Headless smoke test: Bar's islands compile and instantiate (offscreen, no Hyprland, no bridge).
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cat > "$tmp/shell.qml" <<QML
import QtQuick
import Quickshell
import "file:$root/shell/bar"
ShellRoot {
    FloatingWindow {
        implicitWidth: 800; implicitHeight: 60
        SystemIsland { id: sys }
        NotifIsland { x: 500 }
        WorkIsland { id: work; x: 200 }
        TimeIsland { id: time; x: 350 }
        StatusIsland { id: status; x: 450 }
        Component.onCompleted: console.log("LOADED", sys.width > 0, work.width > 0, time.width > 0, status.width > 0)
    }
    Timer { interval: 1500; running: true; onTriggered: Qt.quit() }
}
QML
out=$(env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY QT_QPA_PLATFORM=offscreen timeout 30 qs -p "$tmp/shell.qml" --no-duplicate 2>&1)
echo "$out" | grep -q 'LOADED true true true true' || { echo "FAIL: islands did not load"; echo "$out"; exit 1; }
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
