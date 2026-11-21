#!/usr/bin/env bash
# The clock, weather, system monitor and status islands on a real (headless) Hyprland with the real
# Bar, driven by a virtual pointer (tests/live/tools/uqinput): UK and US dates, the 12- and 24-hour
# clock, Celsius and Fahrenheit, the calendar with today framed, and the VPN lock and its details.
#
#   scripts/barinfo-live.sh
#
# There is no uqd here, so a small harness shell puts the Bar up with a layout of its own and feeds
# it the settings, weather, system monitor reading and public address the bridge would send. The
# VPN is the real services/VpnStatus.qml reading a stub `nmcli` (an OpenVPN connection). With
# UQ_CAPTURE_DIR set (CI does) it keeps a screenshot of each step. Exit 0 when it ran and the checks
# held; 77 when this environment can't run it; 1 when a check failed.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)

skip() { echo "SKIP: $*"; exit 77; }
for bin in Hyprland hyprctl qs dbus-daemon jq socat gcc wayland-scanner; do
    command -v "$bin" >/dev/null 2>&1 || skip "$bin is not installed"
done
ls /dev/dri/card* >/dev/null 2>&1 || skip "no DRM device"
uqctl=${UQCTL:-$root/target/debug/uqctl}
[ -x "$uqctl" ] || skip "no uqctl binary"

tmp=$(mktemp -d /tmp/uqb.XXXXXX)
rt="$tmp/rt"; mkdir -m 700 "$rt"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; wait 2>/dev/null; rm -rf "$tmp"; }
trap cleanup EXIT
fail() { echo "FAIL: $*"; for f in hypr qs; do [ -f "$tmp/$f.log" ] && { echo "--- $f.log"; tail -n 40 "$tmp/$f.log"; }; done; exit 1; }

"$root/tests/live/tools/build-uqinput.sh" "$tmp/uqinput" || skip "uqinput didn't build"

# nmcli: an active OpenVPN connection, its details, and a monitor that never prints.
mkdir -p "$tmp/bin"
cat >"$tmp/bin/nmcli" <<'SH'
#!/bin/sh
case "$*" in
    monitor) exec sleep 3600 ;;
    "-t -f NAME,TYPE con show --active") printf 'Wired connection 1:802-3-ethernet\nProtonVPN NL#12:vpn\n' ;;
    "-t connection show id ProtonVPN NL#12")
        printf '%s\n' 'connection.id:ProtonVPN NL#12' 'connection.type:vpn' \
            'vpn.service-type:org.freedesktop.NetworkManager.openvpn' \
            'vpn.data:remote = nl-12.protonvpn.net:1194, proto-tcp = no, connection-type = password' \
            'GENERAL.DEVICES:tun0' 'IP4.ADDRESS[1]:10.96.0.5/16' 'IP4.GATEWAY:10.96.0.1' ;;
    *) exit 8 ;;
esac
SH
chmod +x "$tmp/bin/nmcli"

# The harness: the real Bar on the first screen, with settings `mode` chooses (uk: UK dates,
# 24-hour clock, Celsius; us: US dates, 12-hour clock, Fahrenheit), set from a file the script
# writes.
mkdir -p "$tmp/harness"
echo uk >"$tmp/mode"
cat >"$tmp/harness/shell.qml" <<QML
import QtQuick
import Quickshell
import Quickshell.Io
import "file:$root/shell/bar"
import "file:$root/shell/core"
ShellRoot {
    id: shellRoot
    Variants {
        id: bars
        model: [Quickshell.screens[0]]
        delegate: Bar {}
    }
    FileView {
        id: modeFile
        path: "$tmp/mode"
        watchChanges: true
        onFileChanged: reload()
    }
    QtObject {
        id: feed
        function fill() {
            var us = modeFile.text().trim() === "us";
            var bar = {
                start: [{ id: "clock", size: "large" }, { id: "weather", size: "large" }],
                center: [{ id: "sysmon", size: "large" }],
                end: [{ id: "status", size: "large" }],
                options: { clock: { date: "long" }, sysmon: { cpu: true, mem: true, gpu: false, temp: true } }
            };
            var shell = { bar: bar, clock_24h: !us, date_format: us ? "us" : "uk", temp_unit: us ? "f" : "c" };
            if (JSON.stringify(Settings.values.shell) !== JSON.stringify(shell)) {
                Settings.values = { shell: shell, privacy: { public_ip_lookup: true } };
                BarLayout.values = { shell: shell };
            }
            Bridge.weather = { temp_c: 18.4, code: 2, is_day: true, location_name: "London",
                today: { max_c: 21, min_c: 11 },
                hourly: [{ time: 1, temp_c: 18, code: 2 }, { time: 3601, temp_c: 17, code: 3 }] };
            Bridge.sysmon = { cpu: 23.4, cores: [12, 30, 25, 18], mem: 41.5, mem_used: 13 * 1073741824,
                mem_total: 32 * 1073741824, temp_c: 54.6, gpu: null };
            Bridge.publicIp = { country_code: "NL", country: "NL", ip: "185.107.56.12", city: "Amsterdam",
                region: "North Holland", org: "AS9009 M247 Europe SRL" };
        }
    }
    Timer { interval: 300; running: true; repeat: true; triggeredOnStart: true; onTriggered: feed.fill() }
    function findNamed(item, name) {
        if (!item)
            return null;
        if (item.objectName === name)
            return item;
        var kids = item.children || [];
        for (var i = 0; i < kids.length; i++) {
            var f = findNamed(kids[i], name);
            if (f)
                return f;
        }
        return null;
    }
    // Where the clock and the VPN lock are on screen, for the script to click.
    Timer {
        interval: 1000; running: true; repeat: true
        onTriggered: {
            var b = bars.instances[0];
            var s = b ? b.slotFor("start:0") : null;
            if (s) {
                var p = s.mapToItem(null, 0, 0);
                console.log("SLOT clock", Math.round(p.x), Math.round(p.y), Math.round(s.width), Math.round(s.height));
            }
            var st = b ? b.slotFor("end:0") : null;
            var lock = st ? shellRoot.findNamed(st, "vpnLock") : null;
            if (lock && lock.visible) {
                var q = lock.mapToItem(null, 0, 0);
                console.log("SLOT vpn", Math.round(q.x), Math.round(q.y), Math.round(lock.width), Math.round(lock.height));
            }
        }
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

cap=${UQ_CAPTURE_DIR:-}
still() { [ -n "$cap" ] && command -v grim >/dev/null 2>&1 && { mkdir -p "$cap"; env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" grim "$cap/barinfo-$1.png" 2>/dev/null; }; }

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
popups() { hc layers -j | jq '[.. | objects | select(.namespace? != null) | .namespace] | map(select(test("popup|island"))) | length'; }
bad=0
check() { echo "FAIL: $*"; bad=1; }

# Where the bar, the clock and the VPN lock are.
for _ in $(seq 1 40); do grep -q 'SLOT vpn' "$tmp/qs.log" && break; sleep 0.25; done
grep 'SLOT' "$tmp/qs.log" | tail -n 2
grep -q 'SLOT vpn' "$tmp/qs.log" || check "the VPN lock never showed"
read -r bx by < <(hc layers -j | jq -r '.. | objects | select(.namespace? == "uq-bar") | "\(.x) \(.y)"' | head -n1)
slot() { grep "SLOT $1 " "$tmp/qs.log" | tail -n1 | sed 's/.*SLOT //'; }
read -r _ kx ky kw kh < <(slot clock)
[ -n "${kx:-}" ] || fail "the harness never reported the clock's place"
click() { send "move $1 $2" "sleep 300" "button 272 down" "sleep 80" "button 272 up" "sleep 900"; drain; }
calendar() {
    click $((bx + kx + kw / 2)) $((by + ky + kh / 2))
    [ "$(popups)" -ge 1 ] || check "no calendar under the clock ($1)"
    still "$2"
    send "key Escape" "sleep 500"; drain
}

sleep 1
still 1-bar-uk
calendar uk 2-calendar-uk
echo us >"$tmp/mode"
sleep 1.5
still 3-bar-us
calendar us 4-calendar-us

# --- the VPN lock and its details ---------------------------------------------------------------
read -r _ vx vy vw vh < <(slot vpn)
if [ -n "${vx:-}" ]; then
    click $((bx + vx + vw / 2)) $((by + vy + vh / 2))
    [ "$(popups)" -ge 1 ] || check "no VPN details under the lock"
    still 5-vpn-details
    send "key Escape" "sleep 500"; drain
fi

grep -qE 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|Cannot read property|is not a function|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? ' "$tmp/qs.log" &&
    { grep -E 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|Cannot read property|is not a function|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? ' "$tmp/qs.log" | head -n 10; check "QML errors"; }
[ -n "$cap" ] && ls -l "$cap"
echo "--- qs.log (tail)"; tail -n 12 "$tmp/qs.log"
[ "$bad" = 0 ] && echo "barinfo-live: all checks held"
exit "$bad"
