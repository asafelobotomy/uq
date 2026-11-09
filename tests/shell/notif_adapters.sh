#!/usr/bin/env bash
# Headless check of NotifBackend over the swaync, mako and dunst adapters (shell/notif/adapters),
# with fake swaync-client, makoctl and dunstctl scripts that print fixed output and log their argv.
# qs runs with PATH holding only the fakes (so a real daemon's client is never found), a private
# XDG_RUNTIME_DIR (no noti-fi socket) and an unusable DBUS_SESSION_BUS_ADDRESS (never the session
# bus). Settings.values is stubbed in the test shell; the bridge is never started.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
qs_bin=$(command -v qs 2>/dev/null) || { echo "notif_adapters skipped: qs not found"; exit 0; }
sleep_bin=$(command -v sleep)
bash_bin=$(command -v bash)
timeout_bin=$(command -v timeout)
tmp=$(mktemp -d)
qs_pid=""
cleanup() {
    [ -n "$qs_pid" ] && kill "$qs_pid" 2>/dev/null && wait "$qs_pid" 2>/dev/null
    pkill -f "$tmp/bin/" 2>/dev/null
    rm -rf "$tmp"
}
trap cleanup EXIT
mkdir -p "$tmp/bin" "$tmp/run" "$tmp/cfg"
chmod 700 "$tmp/run"
log="$tmp/argv.log"

cat > "$tmp/bin/swaync-client" <<SH
#!$bash_bin
echo "swaync-client \$*" >> "$log"
case "\$1" in
    --version) echo "swaync-client 0.10.1" ;;
    -swb) echo '{"text":"3","alt":"notification","tooltip":"","class":"notification"}'
          printf '{"count":99,"pad":"%05000d"}\n' 0
          exec -a "$tmp/bin/swaync-sub" "$sleep_bin" 20 ;;
esac
exit 0
SH
cat > "$tmp/bin/makoctl" <<SH
#!$bash_bin
echo "makoctl \$*" >> "$log"
case "\$*" in
    --help) echo "Usage: makoctl <command> [options...]" ;;
    list) printf 'Notification 12: Hello there\n  App name: Foo\n  Urgency: normal\nNotification 11: Older\n  App name: Bar\n  Urgency: normal\n' ;;
    mode) echo "default" ;;
esac
exit 0
SH
cat > "$tmp/bin/dunstctl" <<SH
#!$bash_bin
echo "dunstctl \$*" >> "$log"
case "\$*" in
    is-paused) echo "false" ;;
    count) printf '              Waiting: 1\n  Currently displayed: 2\n              History: 5\n' ;;
    history-pop) [ -e "$tmp/wedge" ] && { trap '' TERM; exec -a "$tmp/bin/wedged" "$sleep_bin" 30; } ;;
esac
exit 0
SH
chmod +x "$tmp/bin/"*

# run <backend> <bin dir> [after ms] [extra QML]: one shell whose stubbed settings pick <backend>;
# logs STATE after 1 s, then setDnd(true) and toggleCentre(), then AFTER (default 2.5 s) once the
# writes have been re-polled.
run() {
    cat > "$tmp/cfg/shell.qml" <<QML
import QtQuick
import Quickshell
import "file:$root/shell/core"
import "file:$root/shell/notif"
ShellRoot {
    function show(tag) {
        var b = NotifBackend;
        console.log(tag, b.backend, b.available, b.count, b.dnd, b.hasCentre,
            b.latest ? b.latest.appName + "/" + b.latest.summary : "none", "[" + b.warning + "]");
    }
    // Singletons are created on first use: touch NotifBackend now so detection starts at once.
    Component.onCompleted: {
        Settings.values = { notif: { backend: "$1" } };
        console.log("START", NotifBackend.backend);
    }
    Timer {
        interval: 1000; running: true
        onTriggered: { show("STATE"); NotifBackend.setDnd(true); NotifBackend.toggleCentre(); }
    }
    Timer { interval: ${3:-2500}; running: true; onTriggered: { show("AFTER"); Qt.quit(); } }
    ${4:-}
}
QML
    : > "$log"
    env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY QT_QPA_PLATFORM=offscreen \
        PATH="$2" XDG_RUNTIME_DIR="$tmp/run" DBUS_SESSION_BUS_ADDRESS="unix:path=$tmp/nobus" \
        "$timeout_bin" 20 "$qs_bin" -p "$tmp/cfg/shell.qml" --no-duplicate > "$tmp/out" 2>&1 &
    qs_pid=$!
    wait "$qs_pid"
    qs_pid=""
    pkill -f "$tmp/bin/" 2>/dev/null
    if grep -E 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|is not a function|Cannot read property|Syntax[Ee]rror|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? ' "$tmp/out"; then
        echo "FAIL ($1): QML errors"; cat "$tmp/out"; exit 1
    fi
}
expect() {  # expect <label> <regex> <file>
    grep -qE "$2" "$3" || { echo "FAIL: $1"; echo "--- shell:"; cat "$tmp/out"; echo "--- argv:"; cat "$log"; exit 1; }
}

run swaync "$tmp/bin"
expect "swaync count from -swb" 'STATE swaync true 3 false true none \[\]' "$tmp/out"
expect "swaync detected with --version" '^swaync-client --version$' "$log"
expect "swaync subscribed with -swb" '^swaync-client -swb$' "$log"
expect "swaync DnD on with -dn" '^swaync-client -dn$' "$log"
expect "swaync centre with -t" '^swaync-client -t$' "$log"

run mako "$tmp/bin"
expect "mako count from makoctl list" 'STATE mako true 2 false false Foo/Hello there \[\]' "$tmp/out"
expect "mako detected with --help" '^makoctl --help$' "$log"
expect "mako DnD on" '^makoctl mode -a do-not-disturb$' "$log"
n=$(grep -c '^makoctl list$' "$log")
[ "$n" -ge 2 ] || { echo "FAIL: mako not re-polled after the DnD write"; cat "$log"; exit 1; }

run dunst "$tmp/bin"
expect "dunst count" 'STATE dunst true 3 false true none \[\]' "$tmp/out"
expect "dunst DnD on" '^dunstctl set-paused true$' "$log"
expect "dunst centre with history-pop" '^dunstctl history-pop$' "$log"
n=$(grep -c '^dunstctl count$' "$log")
[ "$n" -ge 2 ] || { echo "FAIL: dunst not re-polled after a write"; cat "$log"; exit 1; }

# A dunstctl that ignores SIGTERM: terminated at 5 s, killed and abandoned at 6 s, and the queue
# goes on to the re-poll behind it.
touch "$tmp/wedge"
run dunst "$tmp/bin" 8000
rm -f "$tmp/wedge"
expect "re-poll after a wedged history-pop" '^dunstctl history-pop$' "$log"
tail -n 1 "$log" | grep -qE '^dunstctl (count|is-paused)$' || { echo "FAIL: queue stalled behind a wedged command"; cat "$log"; exit 1; }
pgrep -f "$tmp/bin/wedged" >/dev/null && { echo "FAIL: wedged command not killed"; exit 1; }

# Ruling E: noti-fi drops under auto → pending for a fresh grace window, then built-in. The
# startup grace ends at 5 s; the drop at 4 s restarts it, so MID (6 s) is still pending.
run auto "$tmp/bin" 9500 '
    Timer { interval: 1200; running: true; onTriggered: { Notifi.available = true; show("UP"); } }
    Timer { interval: 4000; running: true; onTriggered: { Notifi.available = false; show("DROP"); } }
    Timer { interval: 6000; running: true; onTriggered: show("MID") }'
expect "notifi while up" '^.*UP notifi ' "$tmp/out"
expect "pending right after the drop" 'DROP pending false' "$tmp/out"
expect "still pending after the startup grace" 'MID pending false' "$tmp/out"
expect "built-in after the new grace" 'AFTER builtin ' "$tmp/out"

# The chosen daemon is missing: built-in, with the warning logged once.
mkdir -p "$tmp/empty"
run dunst "$tmp/empty"
expect "missing dunst falls back to built-in" 'STATE builtin false 0 false true none \[dunst is not installed\]' "$tmp/out"
n=$(grep -c 'dunst is not installed' "$tmp/out")
[ "$n" -eq 3 ] || { echo "FAIL: warning not logged exactly once ($n lines, 2 are STATE/AFTER)"; cat "$tmp/out"; exit 1; }

[ -n "${SHOW:-}" ] && cat "$tmp/out"
echo "notif_adapters ok"
