#!/usr/bin/env bash
# Headless check of NotifBackend over the swaync, mako and dunst adapters (shell/notif/adapters),
# with fake swaync-client, makoctl and dunstctl scripts that print fixed output and log their argv,
# and (for the change-event runs) a fake gdbus whose `monitor` plays a per-run script.
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
    pkill -f "$tmp/g?bin/" 2>/dev/null
    rm -rf "$tmp"
}
trap cleanup EXIT
mkdir -p "$tmp/bin" "$tmp/gbin" "$tmp/run" "$tmp/cfg"
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
    mode) [ -e "$tmp/modes" ] && echo "\$(<"$tmp/modes")" || echo "default" ;;
esac
exit 0
SH
cat > "$tmp/bin/dunstctl" <<SH
#!$bash_bin
echo "dunstctl \$*" >> "$log"
case "\$*" in
    is-paused) [ -e "$tmp/paused" ] && echo "\$(<"$tmp/paused")" || echo "false" ;;
    count) printf '              Waiting: 1\n  Currently displayed: 2\n              History: 5\n' ;;
    history-pop) [ -e "$tmp/wedge" ] && { trap '' TERM; exec -a "$tmp/bin/wedged" "$sleep_bin" 30; } ;;
esac
exit 0
SH
# gdbus: GetServerInformation prints $tmp/info, Introspect $tmp/xml (each fails when missing);
# monitor runs $tmp/monitor.sh.
cat > "$tmp/gbin/gdbus" <<SH
#!$bash_bin
echo "gdbus \$*" >> "$log"
case "\$1" in
    call) case "\$*" in
        *GetServerInformation*) [ -e "$tmp/info" ] && echo "\$(<"$tmp/info")" && exit 0 ;;
        *Introspect*) [ -e "$tmp/xml" ] && echo "\$(<"$tmp/xml")" && exit 0 ;;
    esac
    exit 1 ;;
    monitor) . "$tmp/monitor.sh" ;;
esac
exit 0
SH
chmod +x "$tmp/bin/"* "$tmp/gbin/"*

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
    pkill -f "$tmp/g?bin/" 2>/dev/null
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

# --- change events: a gdbus monitor replaces the 5 s poll for what the daemon signals ---
# after <label> <regex>: fails if a log line after MARK matches.
after_mark() {
    if sed -n '/^MARK$/,$p' "$log" | grep -qE "$2"; then
        echo "FAIL: $1"; echo "--- argv:"; cat "$log"; exit 1
    fi
}
owner='echo "Monitoring signals on object $1 owned by org.freedesktop.Notifications"
echo "The name org.freedesktop.Notifications is owned by :1.7"'
hold="exec -a \"$tmp/bin/gdbus-mon\" \"$sleep_bin\" 20"
dunst_paused="/org/freedesktop/Notifications: org.freedesktop.DBus.Properties.PropertiesChanged ('org.dunstproject.cmd0', {'paused': <true>, 'pauseLevel': <uint32 100>}, @as [])"
event_show='Timer { interval: 2500; running: true; onTriggered: show("EVENT") }'
reset_events() { rm -f "$tmp/info" "$tmp/xml" "$tmp/monitor.sh" "$tmp/paused" "$tmp/modes" "$tmp/restarted"; }

# dunst 1.13.2 signals paused and the counts: DnD set outside uQ at 1.5 s shows at 2.5 s (the
# write at 1 s does not touch the fake's state), and nothing is polled once the monitor is up.
reset_events
echo "('dunst', 'knopwob', '1.13.2 (2026-03-18)', '1.2')" > "$tmp/info"
cat > "$tmp/monitor.sh" <<SH
${owner//\$1//org/freedesktop/Notifications}
"$sleep_bin" 1.5
echo true > "$tmp/paused"
echo "$dunst_paused"
"$sleep_bin" 1.5
echo MARK >> "$log"
$hold
SH
run dunst "$tmp/bin:$tmp/gbin" 7500 "$event_show"
expect "dunst version asked" '^gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.GetServerInformation$' "$log"
expect "dunst monitor started" '^gdbus monitor --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications$' "$log"
expect "dunst DnD from the change signal" 'EVENT dunst true 3 true true none \[\]' "$tmp/out"
expect "monitor reached MARK" '^MARK$' "$log"
after_mark "dunst polled while the monitor was up" '^dunstctl (count|is-paused)$'

# dunst 1.12 signals the counts (since 1.10) but not paused: the monitor runs and only
# is-paused is polled.
reset_events
echo "('dunst', 'knopwob', '1.12.2', '1.2')" > "$tmp/info"
cat > "$tmp/monitor.sh" <<SH
${owner//\$1//org/freedesktop/Notifications}
"$sleep_bin" 2
echo MARK >> "$log"
$hold
SH
run dunst "$tmp/bin:$tmp/gbin" 7500
expect "dunst 1.12 monitor started" '^gdbus monitor ' "$log"
after_mark "dunst 1.12 count polled while the monitor was up" '^dunstctl count$'
sed -n '/^MARK$/,$p' "$log" | grep -qE '^dunstctl is-paused$' || { echo "FAIL: dunst 1.12 is-paused not polled"; cat "$log"; exit 1; }

# dunst 1.9 signals neither: no monitor, plain poll.
reset_events
echo "('dunst', 'knopwob', '1.9.2', '1.2')" > "$tmp/info"
run dunst "$tmp/bin:$tmp/gbin" 6000
grep -q '^gdbus monitor' "$log" && { echo "FAIL: monitor started for dunst 1.9"; cat "$log"; exit 1; }
n=$(grep -c '^dunstctl count$' "$log")
[ "$n" -ge 4 ] || { echo "FAIL: dunst 1.9 not polled ($n)"; cat "$log"; exit 1; }

# The monitor exits at 2 s and every restart fails: the 5 s poll comes back.
reset_events
echo "('dunst', 'knopwob', '1.13.2', '1.2')" > "$tmp/info"
cat > "$tmp/monitor.sh" <<SH
[ -e "$tmp/restarted" ] && exit 1
: > "$tmp/restarted"
${owner//\$1//org/freedesktop/Notifications}
"$sleep_bin" 2
echo MARK >> "$log"
exit 0
SH
run dunst "$tmp/bin:$tmp/gbin" 8500
n=$(grep -c '^gdbus monitor' "$log")
[ "$n" -ge 2 ] || { echo "FAIL: monitor not restarted ($n)"; cat "$log"; exit 1; }
sed -n '/^MARK$/,$p' "$log" | grep -qE '^dunstctl count$' || { echo "FAIL: poll not back after the monitor died"; cat "$log"; exit 1; }

# mako 1.10 has the Modes property (signalled) but not Notifications: the mode change at 1.5 s
# shows at 2.5 s; after MARK only `makoctl list` is polled.
reset_events
printf '%s' "('<!DOCTYPE node>\\n<node>\\n <interface name=\"fr.emersion.Mako\">\\n  <property name=\"Modes\" type=\"as\" access=\"read\">\\n  </property>\\n </interface>\\n</node>\\n',)" > "$tmp/xml"
cat > "$tmp/monitor.sh" <<SH
${owner//\$1//fr/emersion/Mako}
"$sleep_bin" 1.5
echo do-not-disturb > "$tmp/modes"
echo "/fr/emersion/Mako: org.freedesktop.DBus.Properties.PropertiesChanged ('fr.emersion.Mako', @a{sv} {}, ['Modes'])"
"$sleep_bin" 1.5
echo MARK >> "$log"
$hold
SH
run mako "$tmp/bin:$tmp/gbin" 7500 "$event_show"
expect "mako introspected" '^gdbus call --session --dest org.freedesktop.Notifications --object-path /fr/emersion/Mako --method org.freedesktop.DBus.Introspectable.Introspect$' "$log"
expect "mako monitor started" '^gdbus monitor --session --dest org.freedesktop.Notifications --object-path /fr/emersion/Mako$' "$log"
expect "mako DnD from the change signal" 'EVENT mako true 2 true false Foo/Hello there \[\]' "$tmp/out"
after_mark "mako mode polled while the monitor was up" '^makoctl mode$'
sed -n '/^MARK$/,$p' "$log" | grep -qE '^makoctl list$' || { echo "FAIL: mako 1.10 list not polled"; cat "$log"; exit 1; }
reset_events

[ -n "${SHOW:-}" ] && cat "$tmp/out"
echo "notif_adapters ok"
