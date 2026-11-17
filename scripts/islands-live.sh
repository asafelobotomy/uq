#!/usr/bin/env bash
# The System monitor and Storage cleanup islands, and their popups, on a real (headless) Hyprland
# with the real Bar, driven by a virtual pointer (tests/live/tools/uqinput).
#
#   scripts/islands-live.sh
#
# There is no uqd here, so a small harness shell (written to a temp dir) puts the Bar up with a
# layout of its own and feeds the System monitor the reading the bridge would send, with an NVIDIA
# card in it (what `nvidia-smi` stands for; the parsing is covered by uqctl's own tests). The
# Storage island runs its real command, `crumbs scan --json`, which a stub answers. With
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

tmp=$(mktemp -d /tmp/uqi.XXXXXX)
rt="$tmp/rt"; mkdir -m 700 "$rt"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; wait 2>/dev/null; rm -rf "$tmp"; }
trap cleanup EXIT
fail() { echo "FAIL: $*"; for f in hypr qs; do [ -f "$tmp/$f.log" ] && { echo "--- $f.log"; tail -n 40 "$tmp/$f.log"; }; done; exit 1; }

"$root/tests/live/tools/build-uqinput.sh" "$tmp/uqinput" || skip "uqinput didn't build"

# crumbs: the scan the island runs, with something to free.
mkdir -p "$tmp/bin"
cat >"$tmp/bin/crumbs" <<'SH'
#!/bin/sh
cat <<'JSON'
{"schema_version":1,"command":"scan","data":{"categories":[
 {"category":"cache","items":[],"total":6442450944,"unknown_sizes":0,"error":null,"skipped":false},
 {"category":"browser-cache","items":[],"total":2254857830,"unknown_sizes":0,"error":null,"skipped":false},
 {"category":"thumbnails","items":[],"total":734003200,"unknown_sizes":0,"error":null,"skipped":false},
 {"category":"logs","items":[],"total":52428800,"unknown_sizes":0,"error":null,"skipped":false},
 {"category":"trash","items":[],"total":9999999999,"unknown_sizes":0,"error":null,"skipped":false}]}}
JSON
SH
chmod +x "$tmp/bin/crumbs"

# The harness: the real Bar on the first screen, a layout and a System monitor reading of its own.
mkdir -p "$tmp/harness"
cat >"$tmp/harness/shell.qml" <<QML
import QtQuick
import Quickshell
import "file:$root/shell/bar"
import "file:$root/shell/core"
ShellRoot {
    id: shellRoot
    Variants {
        id: bars
        model: [Quickshell.screens[0]]
        delegate: Bar {}
    }
    QtObject {
        id: feed
        function fill() {
            BarLayout.values = { shell: { bar: {
                start: [{ id: "sysmon", size: "custom", units: 12 }, { id: "crumbs", size: "medium" }],
                center: [], end: [],
                options: { sysmon: { cpu: true, mem: true, gpu: true, temp: true }, crumbs: { warn_gb: 5 } }
            } } };
            Bridge.sysmon = { cpu: 37.4, cores: [12, 55, 80, 41, 9, 33, 70, 22], mem: 62.5,
                mem_used: 20 * 1073741824, mem_total: 32 * 1073741824, temp_c: 58.2,
                gpu: { name: "NVIDIA GeForce RTX 4070", busy: 41, vram_used: 3.2 * 1073741824,
                       vram_total: 12 * 1073741824, temp_c: 52 } };
        }
    }
    Timer { interval: 300; running: true; repeat: true; triggeredOnStart: true; onTriggered: feed.fill() }
    // Where each island is on screen, for the script to click.
    Timer {
        interval: 1000; running: true; repeat: true
        onTriggered: {
            var b = bars.instances[0];
            for (var k of ["start:0", "start:1"]) {
                var s = b ? b.slotFor(k) : null;
                if (s) {
                    var p = s.mapToItem(null, 0, 0);
                    console.log("SLOT", k, Math.round(p.x), Math.round(p.y), Math.round(s.width), Math.round(s.height),
                        s.island ? s.island.shown : "-", s.size);
                }
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
still() { [ -n "$cap" ] && command -v grim >/dev/null 2>&1 && { mkdir -p "$cap"; env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" grim "$cap/islands-$1.png" 2>/dev/null; }; }

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

# Where the bar and its islands are: the bar layer's corner plus each island's place in it.
for _ in $(seq 1 40); do grep -q 'SLOT start:1' "$tmp/qs.log" && break; sleep 0.25; done
grep 'SLOT' "$tmp/qs.log" | tail -n 2
read -r bx by < <(hc layers -j | jq -r '.. | objects | select(.namespace? == "uq-bar") | "\(.x) \(.y)"' | head -n1)
slot() { grep "SLOT $1 " "$tmp/qs.log" | tail -n1 | sed 's/.*SLOT //'; }
read -r _ sx sy sw sh _ < <(slot start:0); read -r _ cx cy cw ch _ < <(slot start:1)
[ -n "${sx:-}" ] && [ -n "${cx:-}" ] || fail "the harness never reported the islands' places"
echo "bar at $bx,$by; sysmon $sx,$sy ${sw}x$sh; crumbs $cx,$cy ${cw}x$ch"
sleep 2    # the first `crumbs scan` has answered by now
still 1-bar

click() { send "move $1 $2" "sleep 300" "button 272 down" "sleep 80" "button 272 up" "sleep 900"; drain; }
# --- the System monitor's popup ---------------------------------------------------------------
click $((bx + sx + sw / 2)) $((by + sy + sh / 2))
still 2-sysmon-popup
send "key Escape" "sleep 500"; drain
# --- the Storage cleanup popup ----------------------------------------------------------------
click $((bx + cx + cw / 2)) $((by + cy + ch / 2))
still 3-crumbs-popup
send "key Escape" "sleep 500"; drain

grep -qE 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|Cannot read property|is not a function|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? ' "$tmp/qs.log" &&
    { grep -E 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|Cannot read property|is not a function|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? ' "$tmp/qs.log" | head -n 10; check "QML errors"; }
[ -n "$cap" ] && ls -l "$cap"
echo "--- qs.log (tail)"; tail -n 12 "$tmp/qs.log"
[ "$bad" = 0 ] && echo "islands-live: all checks held"
exit "$bad"
