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
        Component.onCompleted: console.log("LOADED", sys.width > 0)
    }
    Timer { interval: 1500; running: true; onTriggered: Qt.quit() }
}
QML
out=$(env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY QT_QPA_PLATFORM=offscreen timeout 30 qs -p "$tmp/shell.qml" --no-duplicate 2>&1)
echo "$out" | grep -q 'LOADED true' || { echo "FAIL: islands did not load"; echo "$out"; exit 1; }
if echo "$out" | grep -E 'ERROR|TypeError|ReferenceError|is not a type|unavailable' | grep -v 'Hyprland'; then
    echo "FAIL: QML errors"; echo "$out"; exit 1
fi
[ -n "${SHOW:-}" ] && echo "$out"
echo "bar_loads ok"
