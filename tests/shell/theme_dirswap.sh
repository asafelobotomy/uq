#!/usr/bin/env bash
# Headless (offscreen, no layer surfaces) check of the real FileView wiring in ThemeIo: Theme
# must pick up a tokens.json after its whole theme/ directory is swapped.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/st/uq/theme" "$tmp/cfg"
echo '{"colors":{"accent":"#111111"}}' > "$tmp/st/uq/theme/tokens.json"
cat > "$tmp/cfg/shell.qml" <<QML
import QtQuick
import Quickshell
import "file:$root/shell/core"
ShellRoot {
    ThemeIo {}
    Timer { interval: 300; running: true; repeat: true; onTriggered: console.log("ACCENT", Theme.accent) }
    Timer { interval: 7000; running: true; onTriggered: Qt.quit() }
}
QML
(
    sleep 2
    mv "$tmp/st/uq/theme" "$tmp/st/uq/theme.old"
    mkdir "$tmp/st/uq/theme"
    echo '{"colors":{"accent":"#222222"}}' > "$tmp/st/uq/theme/tokens.json"
) &
out=$(env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY XDG_STATE_HOME="$tmp/st" \
    QT_QPA_PLATFORM=offscreen timeout 30 qs -p "$tmp/cfg/shell.qml" --no-duplicate 2>&1)
wait
echo "$out" | grep -q 'ACCENT #111111' || { echo "FAIL: initial tokens not loaded"; echo "$out"; exit 1; }
echo "$out" | grep -q 'ACCENT #222222' || { echo "FAIL: swapped tokens not reloaded"; echo "$out"; exit 1; }
echo "theme_dirswap ok"
