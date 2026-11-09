#!/usr/bin/env bash
# Headless check that Notifi.qml finds noti-fi's control socket on a retry (N2 in
# docs/notes/vm-live-check-results.md): qs starts with no socket, a fake notifd (python, a unix
# socket in a private XDG_RUNTIME_DIR) appears at 2.5 s, goes away at 5 s and comes back at 5.5 s.
# The client must connect (hello, then subscribe) both times, and stay quiet while the socket is
# missing. The session bus and any real noti-fi are never touched.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
qs_bin=$(command -v qs 2>/dev/null) || { echo "notifi_connect skipped: qs not found"; exit 0; }
py=$(command -v python3 2>/dev/null) || { echo "notifi_connect skipped: python3 not found"; exit 0; }
tmp=$(mktemp -d)
pids=()
cleanup() {
    for p in "${pids[@]}"; do kill "$p" 2>/dev/null; wait "$p" 2>/dev/null; done
    rm -rf "$tmp"
}
trap cleanup EXIT
mkdir -p "$tmp/run" "$tmp/cfg"
chmod 700 "$tmp/run"

cat > "$tmp/notifd.py" <<'PY'
import os, socket, sys
path, log = sys.argv[1], sys.argv[2]
try:
    os.unlink(path)
except FileNotFoundError:
    pass
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.bind(path)
s.listen(4)
while True:
    c, _ = s.accept()
    with open(log, "a") as f:
        f.write("accept\n")
    c.sendall(b'{"event":"hello","name":"notifd","protocol":1}\n')
    buf = b""
    while True:
        d = c.recv(4096)
        if not d:
            break
        buf += d
        while b"\n" in buf:
            line, buf = buf.split(b"\n", 1)
            with open(log, "a") as f:
                f.write(line.decode() + "\n")
PY

cat > "$tmp/cfg/shell.qml" <<QML
import QtQuick
import Quickshell
import "file:$root/shell/notif"
ShellRoot {
    Component.onCompleted: console.log("START", Notifi.available)
    Connections {
        target: Notifi
        function onAvailableChanged() { console.log("AVAILABLE", Notifi.available); }
    }
    Timer { interval: 12500; running: true; onTriggered: Qt.quit() }
}
QML

env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY QT_QPA_PLATFORM=offscreen \
    XDG_RUNTIME_DIR="$tmp/run" DBUS_SESSION_BUS_ADDRESS="unix:path=$tmp/nobus" \
    timeout 20 "$qs_bin" -p "$tmp/cfg/shell.qml" --no-duplicate > "$tmp/out" 2>&1 &
qs_pid=$!
pids+=("$qs_pid")
sleep 2.5
"$py" -I "$tmp/notifd.py" "$tmp/run/notifd.sock" "$tmp/log" &
srv=$!
pids+=("$srv")
sleep 2.5
cp "$tmp/out" "$tmp/out-first"
kill "$srv"; wait "$srv" 2>/dev/null
sleep 0.5
"$py" -I "$tmp/notifd.py" "$tmp/run/notifd.sock" "$tmp/log" &
pids+=($!)
wait "$qs_pid"

fail=0
show() { echo "--- shell:"; cat "$tmp/out"; echo "--- notifd:"; cat "$tmp/log" 2>/dev/null; }
if grep -E 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|is not a function|Syntax[Ee]rror' "$tmp/out"; then
    echo "FAIL: QML errors"; fail=1
fi
grep -q 'START false' "$tmp/out" || { echo "FAIL: shell did not start"; fail=1; }
grep -q 'AVAILABLE true' "$tmp/out-first" || { echo "FAIL: no connection within 2.5 s of the socket appearing"; fail=1; }
n=$(grep -c '"method":"subscribe"' "$tmp/log" 2>/dev/null); n=${n:-0}
[ "$n" -ge 2 ] || { echo "FAIL: subscribed $n times, want 2 (connect, then reconnect)"; fail=1; }
[ "$(grep -c 'AVAILABLE true' "$tmp/out")" -ge 2 ] || { echo "FAIL: not available again after the daemon came back"; fail=1; }
# Quiet while the socket was missing: no warning about it before the daemon first appeared.
if grep -E 'notifd\.sock|Read of|Not a file|No such file' "$tmp/out-first"; then
    echo "FAIL: noisy while the socket was missing"; fail=1
fi
[ "$fail" = 0 ] || show
[ "$fail" = 0 ] && echo "notifi_connect ok"
exit "$fail"
