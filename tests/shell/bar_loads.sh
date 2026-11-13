#!/usr/bin/env bash
# Headless smoke test: every bar island, at every size, and the launcher and OSD routes (global
# shortcuts, `uq:edit-bar` among them, and the `uq` IPC handler) compile and instantiate
# (offscreen, no Hyprland, no bridge), and each island is exactly as wide as its size: 1, 2, 4 or
# 6 times the Icon size. The bar itself, its popups and Spotlight are PanelWindows, which need a
# Wayland backend; smoke-shell.sh covers them.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cat > "$tmp/shell.qml" <<QML
import QtQuick
import Quickshell
import "file:$root/shell/bar"
import "file:$root/shell/bar/islands"
import "file:$root/shell/launcher"
import "file:$root/shell/osd"
import "file:$root/shell/core"
ShellRoot {
    LauncherRoutes { id: routes }
    OsdRoutes { id: osd }
    FloatingWindow {
        implicitWidth: 1600; implicitHeight: 200
        Column {
            Repeater {
                model: ["icon", "small", "medium", "large"]
                delegate: Row {
                    id: sizeRow
                    required property string modelData
                    readonly property var widths: [sys.width, work.width, clock.width, notif.width, status.width,
                        weather.width, stocks.width, news.width, sysmon.width, world.width, media.width, kbd.width]
                    SystemIsland { id: sys; size: sizeRow.modelData }
                    WorkIsland { id: work; size: sizeRow.modelData }
                    ClockIsland { id: clock; size: sizeRow.modelData }
                    NotifIsland { id: notif; size: sizeRow.modelData }
                    StatusIsland { id: status; size: sizeRow.modelData }
                    WeatherIsland { id: weather; size: sizeRow.modelData }
                    StocksIsland { id: stocks; size: sizeRow.modelData }
                    NewsIsland { id: news; size: sizeRow.modelData }
                    SysMonIsland { id: sysmon; size: sizeRow.modelData }
                    WorldClockIsland { id: world; size: sizeRow.modelData }
                    MediaIsland { id: media; size: sizeRow.modelData }
                    KeyboardIsland { id: kbd; size: sizeRow.modelData }
                    SeparatorIsland {}
                    Component.onCompleted: {
                        var unit = { icon: 1, small: 2, medium: 4, large: 6 }[modelData] * BarLayout.unit;
                        console.log("SIZE", modelData, widths.every(function (w) { return w === unit; }), widths.join(","));
                    }
                }
            }
        }
        Component.onCompleted: console.log("LOADED", routes.routed, osd.routed)
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
echo "$out" | grep -q 'LOADED true true' || { echo "FAIL: islands or launcher routes did not load"; echo "$out"; exit 1; }
for s in icon small medium large; do
    echo "$out" | grep -q "SIZE $s true" || { echo "FAIL: an island isn't as wide as its size ($s)"; echo "$out" | grep SIZE; exit 1; }
done
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
