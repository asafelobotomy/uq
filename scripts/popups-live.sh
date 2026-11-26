#!/usr/bin/env bash
# Every bar popup on a real (headless) Hyprland with the real Bar, quick settings and the built-in
# notification centre, driven by a virtual keyboard and pointer (tests/live/tools/uqinput): each
# island's popup opens on a click, takes the keyboard (Esc closes it) and closes on a second click;
# one popup at a time, also against quick settings and the centre opened by key; the island menu;
# the VPN lock; and other apps' tray icons (tests/live/fixtures/tray-app.qml): a click activates,
# a right click opens the app's menu, and the "+N" list opens and activates the rest.
#
#   scripts/popups-live.sh
#
# There is no uqd here, so a harness shell puts the Bar up with a layout of its own and feeds it
# the readings the bridge would send; crumbs, oyst-cli and nmcli are stubs. With UQ_CAPTURE_DIR
# set (CI does) it keeps a screenshot of each step. Exit 0 when it ran and the checks held; 77 when
# this environment can't run it; 1 when a check failed.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)

skip() { echo "SKIP: $*"; exit 77; }
for bin in Hyprland hyprctl qs dbus-daemon jq socat gcc wayland-scanner; do
    command -v "$bin" >/dev/null 2>&1 || skip "$bin is not installed"
done
ls /dev/dri/card* >/dev/null 2>&1 || skip "no DRM device"
qmlbin=$(command -v qml6 || command -v qml) || skip "qml6 is not installed"
uqctl=${UQCTL:-$root/target/debug/uqctl}
[ -x "$uqctl" ] || skip "no uqctl binary"

tmp=$(mktemp -d /tmp/uqp.XXXXXX)
rt="$tmp/rt"; mkdir -m 700 "$rt"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; wait 2>/dev/null; rm -rf "$tmp"; }
trap cleanup EXIT
fail() { echo "FAIL: $*"; for f in hypr qs; do [ -f "$tmp/$f.log" ] && { echo "--- $f.log"; tail -n 40 "$tmp/$f.log"; }; done; exit 1; }

"$root/tests/live/tools/build-uqinput.sh" "$tmp/uqinput" || skip "uqinput didn't build"

# Stubs: storage cleanup, oysterAV and a NetworkManager with a VPN up.
mkdir -p "$tmp/bin" "$tmp/state"
cat >"$tmp/bin/crumbs" <<'SH'
#!/bin/sh
echo '{"schema_version":1,"command":"scan","data":{"categories":[{"category":"cache","items":[],"total":6442450944,"unknown_sizes":0,"error":null,"skipped":false}]}}'
SH
cat >"$tmp/bin/oyst-cli" <<'SH'
#!/bin/sh
[ "$1 $2" = "status summary" ] || exit 0
echo '{"schema":1,"state":"ok","reasons":[],"scanning":false,"scan_progress":null,"last_scan_age_hours":2,"open_findings":0,"signature_age_hours":3,"signatures_stale":false,"quarantine_count":0,"checks":[],"setup_completed":true}'
SH
cat >"$tmp/bin/nmcli" <<'SH'
#!/bin/sh
case "$*" in
*monitor*) exec sleep 100000 ;;
*--active*) printf 'Work VPN:vpn\nwired:802-3-ethernet\n' ;;
*) printf 'connection.id:Work VPN\nvpn.service-type:org.freedesktop.NetworkManager.openvpn\n' ;;
esac
SH
chmod +x "$tmp/bin"/*

mkdir -p "$tmp/harness"
cat >"$tmp/harness/shell.qml" <<QML
//@ pragma UseQApplication
import QtQuick
import Quickshell
import Quickshell.Io
import "file:$root/shell/bar"
import "file:$root/shell/core"
import "file:$root/shell/quick"
import "file:$root/shell/notif"
import "file:$root/shell/notif/builtin"
ShellRoot {
    id: shellRoot
    readonly property var backend: NotifBackend
    Variants {
        id: bars
        model: [Quickshell.screens[0]]
        delegate: Bar {}
    }
    QuickSettings {}
    Centre {}
    function fill() {
        BarLayout.values = { shell: { bar: {
            start: [{ id: "clock", size: "medium" }, { id: "sysmon", size: "small" }],
            center: [{ id: "crumbs", size: "small" }, { id: "security", size: "small" }],
            end: [{ id: "tray", size: "small" }, { id: "notif", size: "small" }, { id: "status", size: "large" }],
            options: {}
        } } };
        Bridge.sysmon = { cpu: 37.4, cores: [12, 55, 80, 41], mem: 62.5, mem_used: 20 * 1073741824,
            mem_total: 32 * 1073741824, temp_c: 58 };
        Bridge.notifOwner = { known: true, owner: null, self: false };
    }
    Timer { interval: 300; running: true; repeat: true; triggeredOnStart: true; onTriggered: shellRoot.fill() }
    // What is open, on every change; and where each island is, for the script to click.
    property string state: ""
    Timer {
        interval: 100; running: true; repeat: true
        onTriggered: {
            var s = Surfaces.open + " " + (BarLayout.activePopup ? "popup" : "none");
            if (s !== shellRoot.state) {
                shellRoot.state = s;
                console.log("STATE", s);
            }
        }
    }
    Timer {
        interval: 1000; running: true; repeat: true
        onTriggered: {
            var b = bars.instances[0];
            for (var i = 0; b && i < b.items.length; i++) {
                var s = b.slotFor(b.items[i].key);
                if (s && s.visible) {
                    var p = s.mapToItem(null, 0, 0);
                    console.log("SLOT", b.items[i].id, Math.round(p.x), Math.round(p.y), Math.round(s.width), Math.round(s.height));
                }
            }
            if (b)
                shellRoot.report(b.contentItem);
        }
    }
    // Buttons inside the islands, by objectName: the first of each.
    function report(item) {
        var seen = {}, queue = [item];
        while (queue.length > 0) {
            var it = queue.shift();
            if (["quickButton", "vpnLock", "trayIcon", "trayMore"].indexOf(it.objectName) >= 0 && !seen[it.objectName]
                    && it.visible && it.width > 0) {
                seen[it.objectName] = true;
                var p = it.mapToItem(null, 0, 0);
                console.log("SLOT", it.objectName, Math.round(p.x), Math.round(p.y), Math.round(it.width), Math.round(it.height));
            }
            for (var i = 0; i < it.children.length; i++)
                queue.push(it.children[i]);
        }
    }
    // What a keybind does (Super+A, Super+N): Surfaces with no anchor.
    IpcHandler {
        target: "t"
        function open(name: string): string { Surfaces.toggle(name, null, null); return "ok"; }
    }
}
QML

dbus-daemon --session --address="unix:path=$tmp/bus" --nofork --nopidfile >"$tmp/dbus.log" 2>&1 &
pids+=($!)
cat >"$tmp/hypr.lua" <<LUA
require([[$root/config/hypr/base.lua]])
hl.monitor({ output = "", mode = "1280x720", position = "auto", scale = 1 })
hl.config({ misc = { disable_hyprland_logo = true, disable_splash_rendering = true } })
LUA
env XDG_RUNTIME_DIR="$rt" LIBGL_ALWAYS_SOFTWARE=1 UQ_DEV_SESSION=1 Hyprland -c "$tmp/hypr.lua" >"$tmp/hypr.log" 2>&1 &
pids+=($!)
sig=""
for _ in $(seq 1 50); do
    sig=$(find "$rt/hypr" -mindepth 1 -maxdepth 1 -printf '%f\n' 2>/dev/null | head -n1)
    [ -n "$sig" ] && [ -S "$rt/hypr/$sig/.socket.sock" ] && break
    sleep 0.2
done
[ -n "$sig" ] || skip "Hyprland never opened its socket"
hc() { env XDG_RUNTIME_DIR="$rt" HYPRLAND_INSTANCE_SIGNATURE="$sig" hyprctl "$@"; }
wl=""
for _ in $(seq 1 25); do
    wl=$(find "$rt" -maxdepth 1 -name 'wayland-*' ! -name '*.lock' -printf '%f\n' | head -n1)
    [ -n "$wl" ] && break; sleep 0.2
done
[ -n "$wl" ] || skip "no Wayland socket"
[ -n "$(hc monitors -j | jq -r '.[0].name // empty')" ] || hc output create headless >/dev/null 2>&1
sleep 0.5

shell_env=(env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" HYPRLAND_INSTANCE_SIGNATURE="$sig"
    DBUS_SESSION_BUS_ADDRESS="unix:path=$tmp/bus" PATH="$tmp/bin:$(dirname "$uqctl"):$PATH"
    XDG_STATE_HOME="$tmp/state" QT_QPA_PLATFORM=wayland LC_ALL=C.UTF-8)
"${shell_env[@]}" qs -p "$tmp/harness/shell.qml" --no-duplicate >"$tmp/qs.log" 2>&1 &
pids+=($!)
for _ in $(seq 1 50); do hc layers -j | grep -q uq-bar && break; sleep 0.2; done
hc layers -j | grep -q uq-bar || fail "the bar never appeared"
for _ in $(seq 1 15); do
    a=$(hc clients -j | jq -r '.[] | select(.class | test("hyprland")) | .address' | head -n1)
    [ -n "$a" ] && { hc dispatch "hl.dsp.window.close({ window = \"address:$a\" })" >/dev/null; sleep 0.5; break; }
    sleep 0.2
done
# Four apps with tray icons: the Tray island (Small) shows one and lists the rest under "+3".
for n in alpha beta gamma delta; do
    "${shell_env[@]}" QT_QPA_PLATFORM=offscreen "$qmlbin" "$root/tests/live/fixtures/tray-app.qml" -- "$n" \
        "$root/tests/live/fixtures/oysterav.png" >>"$tmp/tray.log" 2>&1 &
    pids+=($!)
done

cap=${UQ_CAPTURE_DIR:-}
still() { [ -n "$cap" ] && command -v grim >/dev/null 2>&1 && { mkdir -p "$cap"; env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" grim "$cap/popups-$1.png" 2>/dev/null; }; }

mkfifo "$tmp/in"
env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" "$tmp/uqinput" 1280 720 <"$tmp/in" >"$tmp/uqinput.out" 2>&1 &
pids+=($!)
exec 7>"$tmp/in"
for _ in $(seq 1 50); do grep -q ready "$tmp/uqinput.out" && break; sleep 0.1; done
grep -q ready "$tmp/uqinput.out" || fail "uqinput: $(cat "$tmp/uqinput.out")"
sent=0
send() { sent=$((sent + $#)); printf '%s\n' "$@" >&7; }
drain() {
    for _ in $(seq 1 300); do
        [ "$(grep -c '^ok' "$tmp/uqinput.out")" -ge "$sent" ] && return 0
        sleep 0.1
    done
    fail "uqinput didn't finish its commands ($(tail -n 3 "$tmp/uqinput.out"))"
}
bad=0
check() { echo "FAIL: $*"; bad=1; }
ipc() { "${shell_env[@]}" timeout 10 qs ipc -p "$tmp/harness/shell.qml" call t open "$1" >/dev/null 2>&1; }
state() { grep 'STATE ' "$tmp/qs.log" | tail -n1 | sed 's/.*STATE //'; }
popups() { hc layers -j | jq '[.. | objects | select(.namespace? == "uq-bar-popup")] | length'; }
expect() {   # expect "WHAT" "SURFACE POPUP"
    for _ in $(seq 1 15); do [ "$(state)" = "$2" ] && { echo "ok: $1 ($2)"; return 0; }; sleep 0.1; done
    check "$1: expected '$2', got '$(state)'"
}

for _ in $(seq 1 40); do grep -q 'SLOT status' "$tmp/qs.log" && break; sleep 0.25; done
sleep 2    # crumbs and oyst-cli have answered and the tray icons have arrived by now
read -r bx by < <(hc layers -j | jq -r '.. | objects | select(.namespace? == "uq-bar") | "\(.x) \(.y)"' | head -n1)
slot() { grep "SLOT $1 " "$tmp/qs.log" | tail -n1 | sed 's/.*SLOT [A-Za-z]* //'; }
grep 'SLOT' "$tmp/qs.log" | sed 's/.*SLOT //' | sort -u -k1,1
centre() { read -r x y w h < <(slot "$1"); echo "$((bx + x + w / 2)) $((by + y + h / 2))"; }
click() { send "move $1 $2" "sleep 250" "button ${3:-272} down" "sleep 80" "button ${3:-272} up" "sleep 800"; drain; }
esc() { send "key Escape" "sleep 500"; drain; }
still 1-bar

# --- each island's popup: opens on a click, has the keyboard, toggles on a second click --------
for id in clock sysmon crumbs security; do
    [ -n "$(slot $id)" ] || { check "$id: the island is not on the bar"; continue; }
    click $(centre $id)
    expect "$id popup opens" "closed popup"
    still "2-$id"
    esc
    expect "$id popup closes on Esc" "closed none"
    click $(centre $id); click $(centre $id)
    expect "$id popup closes on a second click" "closed none"
done
# --- one popup at a time ---------------------------------------------------------------------
click $(centre clock); click $(centre sysmon)
expect "clock then system monitor" "closed popup"
[ "$(popups)" = 1 ] || check "clock then system monitor: $(popups) popups mapped"
still 3-swap
esc
# --- the island menu (right click) -------------------------------------------------------------
click $(centre crumbs) 273
expect "island menu opens" "closed popup"
still 4-island-menu
esc
expect "island menu closes on Esc" "closed none"

# --- quick settings: its button, Esc, a second click; then against an island popup -----------------
read -r qx qy < <(centre quickButton)
click $qx $qy
expect "quick settings opens from its button" "quicksettings none"
still 5-quick
esc
expect "quick settings closes on Esc" "closed none"
click $qx $qy; click $qx $qy
expect "quick settings closes on a second click" "closed none"
click $(centre clock)
ipc quicksettings; sleep 0.8
expect "Super+A over the calendar: the calendar goes" "quicksettings none"
[ "$(popups)" = 0 ] || check "Super+A over the calendar: the calendar is still mapped"
still 6-quick-over-popup
esc
expect "quick settings opened over a popup takes the keyboard" "closed none"
ipc quicksettings; sleep 0.8
click $(centre sysmon)
expect "an island clicked while quick settings is open" "closed popup"
esc

# --- the notification centre (built-in): from its island, Esc, a second click -------------------
if [ -n "$(slot notif)" ]; then
    click $(centre notif)
    expect "notification centre opens from its island" "notifcentre none"
    still 7-centre
    esc
    expect "notification centre closes on Esc (it has the keyboard)" "closed none"
    click $(centre notif); click $(centre notif)
    expect "notification centre closes on a second click" "closed none"
else
    check "the Notifications island is not on the bar"
fi

# --- the VPN lock ----------------------------------------------------------------------------
if [ -n "$(slot vpnLock)" ]; then
    click $(centre vpnLock)
    expect "the VPN lock opens its details" "closed popup"
    still 8-vpn
    esc
    expect "VPN details close on Esc" "closed none"
else
    check "the VPN lock never showed"
fi

# --- other apps' tray icons ------------------------------------------------------------------
if [ -z "$(slot trayIcon)" ] || [ -z "$(slot trayMore)" ]; then
    check "the Tray island never showed an icon and '+N' (tray log: $(tail -n 3 "$tmp/tray.log" | tr '\n' '|'))"
else
    read -r tl ty < <(centre trayIcon); read -r tm _ < <(centre trayMore)
    click $tl $ty
    for _ in $(seq 1 20); do grep -q TRAYACT "$tmp/tray.log" && break; sleep 0.1; done
    grep -q TRAYACT "$tmp/tray.log" && echo "ok: a click activates the app" || check "a click on a tray icon didn't activate its app"
    click $tl $ty 273
    still 9-tray-menu
    send "key Down" "key Return" "sleep 600"; drain
    for _ in $(seq 1 20); do grep -q TRAYMENU "$tmp/tray.log" && break; sleep 0.1; done
    grep -q TRAYMENU "$tmp/tray.log" && echo "ok: the app's menu opens and takes the keyboard" \
        || { check "a right click didn't open a working app menu"; esc; }
    send "move 640 400" "sleep 600"; drain
    still 10-tray-tooltip-gone
    click $tm $ty
    expect "the '+N' list opens" "closed popup"
    still 11-tray-more
    before=$(grep -c TRAYACT "$tmp/tray.log")
    send "key Return" "sleep 800"; drain
    expect "Enter on the list closes it" "closed none"
    [ "$(grep -c TRAYACT "$tmp/tray.log")" -gt "$before" ] && echo "ok: Enter activates the first listed app" \
        || check "Enter on the '+N' list didn't activate the app"
fi

errs='TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|Cannot read property|is not a function|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? '
grep -qE "$errs" "$tmp/qs.log" && { grep -E "$errs" "$tmp/qs.log" | head -n 10; check "QML errors"; }
[ -n "$cap" ] && ls -l "$cap" | grep popups-
echo "--- tray.log"; tail -n 8 "$tmp/tray.log"
echo "--- qs.log (tail)"; grep -v -e SLOT -e STATE "$tmp/qs.log" | tail -n 15
[ "$bad" = 0 ] && echo "popups-live: all checks held"
exit "$bad"
