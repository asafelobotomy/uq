#!/usr/bin/env bash
# Headless check of BridgeIo against a fake `uqctl` on PATH (never the real bridge or bus):
# Hello arrives, a call round-trips, and the process is restarted after it exits.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin" "$tmp/cfg"
cat > "$tmp/bin/uqctl" <<'SH'
#!/bin/sh
echo "START" >> "$FAKE_LOG"
echo '{"v":1,"event":"Hello","data":{"v":1,"uqd_up":true,"locked":false}}'
read -r line
id=$(printf '%s' "$line" | sed 's/.*"id":\([0-9]*\).*/\1/')
echo "{\"v\":1,\"id\":$id,\"ok\":\"pong\"}"
sleep 0.3
exit 0
SH
chmod +x "$tmp/bin/uqctl"
cat > "$tmp/cfg/shell.qml" <<QML
import QtQuick
import Quickshell
import "file:$root/shell/core"
ShellRoot {
    BridgeIo {}
    Connections {
        target: Bridge
        function onConnectedChanged() {
            if (Bridge.connected)
                Bridge.call("Ping", []).then(function (v) { console.log("REPLY", v, Bridge.uqdUp); });
        }
    }
    Timer { interval: 5000; running: true; onTriggered: Qt.quit() }
}
QML
out=$(env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY FAKE_LOG="$tmp/log" PATH="$tmp/bin:$PATH" \
    QT_QPA_PLATFORM=offscreen timeout 30 qs -p "$tmp/cfg/shell.qml" --no-duplicate 2>&1)
echo "$out" | grep -q 'REPLY pong true' || { echo "FAIL: no reply"; echo "$out"; exit 1; }
n=$(wc -l < "$tmp/log")
[ "$n" -ge 2 ] || { echo "FAIL: bridge not restarted ($n starts)"; exit 1; }
echo "bridge_io ok ($n starts)"

# Missing binary: Process emits only runningChanged(false); restart/backoff must still run.
mkdir -p "$tmp/empty" "$tmp/cfg2"
cat > "$tmp/cfg2/shell.qml" <<QML
import QtQuick
import Quickshell
import "file:$root/shell/core"
ShellRoot {
    BridgeIo {}
    Timer { interval: 3500; running: true; onTriggered: { console.log("ATTEMPTS", Bridge.attempt); Qt.quit() } }
}
QML
out=$(env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY PATH="$tmp/empty:/usr/bin:/bin" \
    QT_QPA_PLATFORM=offscreen timeout 30 qs -p "$tmp/cfg2/shell.qml" --no-duplicate 2>&1)
a=$(echo "$out" | sed -n 's/.*ATTEMPTS \([0-9]*\).*/\1/p' | tail -1)
[ "${a:-0}" -ge 2 ] || { echo "FAIL: no restart after failed start (attempts=${a:-none})"; echo "$out"; exit 1; }
echo "bridge_io missing-binary ok (attempts=$a)"
