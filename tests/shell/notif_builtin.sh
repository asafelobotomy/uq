#!/usr/bin/env bash
# Headless check of the built-in notification server (shell/notif/Builtin.qml) on a private
# dbus-daemon (never the session bus): it takes org.freedesktop.Notifications when wanted, the
# body reaches the shell and sanitises to the allowed subset, a replacement updates `latest`, a
# transient held back by DnD is not kept, and the store keeps at most 50.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
for tool in notify-send dbus-daemon dbus-send qs; do
    command -v "$tool" >/dev/null 2>&1 || { echo "notif_builtin skipped: $tool not found"; exit 0; }
done
tmp=$(mktemp -d)
bus_pid=""
qs_pid=""
cleanup() {
    [ -n "$qs_pid" ] && kill "$qs_pid" 2>/dev/null && wait "$qs_pid" 2>/dev/null
    [ -n "$bus_pid" ] && kill "$bus_pid" 2>/dev/null
    rm -rf "$tmp"
}
trap cleanup EXIT

# A bus with no service directories, so nothing on it can be activated (no third-party daemon
# ever starts to claim the name). The socket lives under mktemp's short path (< 108 bytes).
cat > "$tmp/bus.conf" <<XML
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path=$tmp/bus</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
XML
bus_pid=$(dbus-daemon --config-file="$tmp/bus.conf" --fork --print-pid) || { echo "FAIL: dbus-daemon did not start"; exit 1; }
export DBUS_SESSION_BUS_ADDRESS="unix:path=$tmp/bus"

mkdir -p "$tmp/cfg"
cat > "$tmp/cfg/shell.qml" <<QML
import QtQuick
import Quickshell
import "file:$root/shell/notif"
import "file:$root/shell/logic/sanitize.js" as S
ShellRoot {
    Connections {
        target: Builtin
        function onRegisteredChanged() { console.log("REGISTERED", Builtin.registered) }
        function onNotificationsChanged() {
            var n = Builtin.notifications[0];
            if (!n)
                return;
            if (n.summary === "done") {
                console.log("STORE", Builtin.notifications.length, Builtin.popups.length, Builtin.count, Builtin.latest.summary);
                Qt.quit();
            } else if (n.summary === "Hi") {
                console.log("NOTIF", Builtin.count, S.sanitize(n.body), Builtin.latest.appName);
            } else if (n.summary === "end") {
                var held = Builtin.notifications.some(function (x) { return x.summary === "tr"; });
                console.log("END", Builtin.notifications.length, Builtin.count, held);
                Builtin.dnd = false;
            }
        }
        // A replacement updates the notification in place: latest follows, the store doesn't grow.
        function onLatestChanged() {
            if (Builtin.latest && Builtin.latest.summary === "replaced" && !Builtin.dnd) {
                console.log("LATEST", Builtin.latest.summary, Builtin.notifications.length, Builtin.count);
                Builtin.dnd = true;
            }
        }
        function onDndChanged() { console.log("DND", Builtin.dnd) }
    }
    Component.onCompleted: Builtin.wanted = true
    // Backstop only: the shell quits once it has logged STORE.
    Timer { interval: 90000; running: true; onTriggered: Qt.quit() }
}
QML
env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY QT_QPA_PLATFORM=offscreen \
    qs -p "$tmp/cfg/shell.qml" --no-duplicate > "$tmp/out" 2>&1 &
qs_pid=$!

owned() {
    dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus \
        org.freedesktop.DBus.NameHasOwner string:org.freedesktop.Notifications 2>/dev/null | grep -q 'boolean true'
}
for _ in $(seq 100); do owned && break; sleep 0.1; done
owned || { echo "FAIL: built-in server never took org.freedesktop.Notifications"; cat "$tmp/out"; exit 1; }

wait_for() {
    for _ in $(seq 100); do grep -q "$1" "$tmp/out" && return 0; sleep 0.1; done
    return 1
}

timeout 5 notify-send -a Test "Hi" "<b>x</b><img src='http://e/x.png'>"
wait_for 'NOTIF ' || { echo "FAIL: no notification reached the shell"; cat "$tmp/out"; exit 1; }
grep -q 'NOTIF 1 <b>x</b> Test$' "$tmp/out" || { echo "FAIL: wrong count or body"; cat "$tmp/out"; exit 1; }

id=$(timeout 5 notify-send -p -a Test "orig" "first")
timeout 5 notify-send -r "$id" -a Test "replaced" "second"
wait_for 'DND true' || { echo "FAIL: a replacement did not update latest"; cat "$tmp/out"; exit 1; }
grep -q 'LATEST replaced 2 2$' "$tmp/out" || { echo "FAIL: replacement grew the store or count"; cat "$tmp/out"; exit 1; }

timeout 5 notify-send --transient -a Test "tr" "held"
timeout 5 notify-send -a Test "end" "kept"
wait_for 'END ' || { echo "FAIL: no notification under DnD reached the shell"; cat "$tmp/out"; exit 1; }
grep -q 'END 3 3 false$' "$tmp/out" || { echo "FAIL: a transient held back by DnD was kept"; cat "$tmp/out"; exit 1; }
wait_for 'DND false' || { echo "FAIL: DnD not cleared"; cat "$tmp/out"; exit 1; }

for i in $(seq 60); do timeout 5 notify-send -a Test "n$i" "body $i"; done
timeout 5 notify-send -a Test "done" "last"
wait_for 'STORE ' || { echo "FAIL: the last notification never arrived"; cat "$tmp/out"; exit 1; }
grep -q 'STORE 50 3 64 done$' "$tmp/out" || { echo "FAIL: store not capped at 50 (or popups at 3)"; cat "$tmp/out"; exit 1; }

if grep -E 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|is not a function|Cannot read property|Syntax[Ee]rror|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? ' "$tmp/out"; then
    echo "FAIL: QML errors"; cat "$tmp/out"; exit 1
fi
[ -n "${SHOW:-}" ] && cat "$tmp/out"
echo "notif_builtin ok"
