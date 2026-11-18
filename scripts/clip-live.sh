#!/usr/bin/env bash
# uqlip's clipboard history on a real (headless) Hyprland: the real uqlip daemon, the real Bar
# with the Clipboard island, and the popup opened by the real SUPER+SHIFT+V shortcut, driven by a
# virtual keyboard and pointer (tests/live/tools/uqinput).
#
#   scripts/clip-live.sh
#
# What it checks: text and an image copied with wl-copy appear in the history, the island shows
# on the bar, the shortcut opens the popup, typing filters, Enter puts the entry back on the
# clipboard (wl-paste reads it) and closes the popup, Ctrl+P pins, Delete forgets, Escape closes,
# and `qs ipc call uq clip` opens it too. With UQ_CAPTURE_DIR set (CI does) it keeps a screenshot
# of each step and a GIF. Exit 0 when it ran and the checks held; 77 when this environment can't
# run it; 1 when a check failed.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)

skip() { echo "SKIP: $*"; exit 77; }
for bin in Hyprland hyprctl qs dbus-daemon jq gcc wayland-scanner wl-copy wl-paste grim; do
    command -v "$bin" >/dev/null 2>&1 || skip "$bin is not installed"
done
ls /dev/dri/card* >/dev/null 2>&1 || skip "no DRM device"
uqctl=${UQCTL:-$root/target/debug/uqctl}
uqlip=${UQLIP:-$(dirname "$uqctl")/uqlip}
[ -x "$uqctl" ] && [ -x "$uqlip" ] || skip "no uqctl or uqlip binary"

tmp=$(mktemp -d /tmp/uqc.XXXXXX)
rt="$tmp/rt"; mkdir -m 700 "$rt"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; wait 2>/dev/null; rm -rf "$tmp"; }
trap cleanup EXIT
fail() { echo "FAIL: $*"; for f in hypr qs uqlip; do [ -f "$tmp/$f.log" ] && { echo "--- $f.log"; tail -n 40 "$tmp/$f.log"; }; done; exit 1; }

"$root/tests/live/tools/build-uqinput.sh" "$tmp/uqinput" || skip "uqinput didn't build"

# The harness: the real Bar (with the Clipboard island), the real shortcut routes and the popup.
mkdir -p "$tmp/harness"
cat >"$tmp/harness/shell.qml" <<QML
import QtQuick
import Quickshell
import "file:$root/shell/bar"
import "file:$root/shell/core"
import "file:$root/shell/launcher"
import "file:$root/shell/clip"
ShellRoot {
    LauncherRoutes {}
    ClipPopup {}
    Variants {
        id: bars
        model: [Quickshell.screens[0]]
        delegate: Bar {}
    }
    QtObject {
        id: feed
        function fill() {
            BarLayout.values = { shell: { bar: { start: [{ id: "clip", size: "small" }], center: [], end: [], options: {} } } };
        }
    }
    Timer { interval: 300; running: true; repeat: true; triggeredOnStart: true; onTriggered: feed.fill() }
    Timer {
        interval: 1000; running: true; repeat: true
        onTriggered: {
            var b = bars.instances[0];
            var s = b ? b.slotFor("start:0") : null;
            if (s) {
                var p = s.mapToItem(null, 0, 0);
                console.log("SLOT", Math.round(p.x), Math.round(p.y), Math.round(s.width), Math.round(s.height), s.island ? s.island.shown : "-");
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

env_all=(env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" HYPRLAND_INSTANCE_SIGNATURE="$sig"
    DBUS_SESSION_BUS_ADDRESS="unix:path=$tmp/bus" PATH="$(dirname "$uqctl"):$PATH"
    XDG_STATE_HOME="$tmp/state" XDG_DATA_HOME="$tmp/data" XDG_CONFIG_HOME="$tmp/cfg"
    QT_QPA_PLATFORM=wayland LC_ALL=C.UTF-8)
"${env_all[@]}" "$uqlip" >"$tmp/uqlip.log" 2>&1 &
pids+=($!)
clip() { "${env_all[@]}" "$uqctl" clip "$@"; }
for _ in $(seq 1 50); do clip status >/dev/null 2>&1 && break; sleep 0.1; done
clip status >/dev/null 2>&1 || fail "uqlip did not start"

"${env_all[@]}" qs -p "$tmp/harness/shell.qml" --no-duplicate >"$tmp/qs.log" 2>&1 &
pids+=($!)
for _ in $(seq 1 50); do hc layers -j | grep -q uq-bar && break; sleep 0.2; done
hc layers -j | grep -q uq-bar || fail "the bar never appeared"
for _ in $(seq 1 15); do
    a=$(hc clients -j | jq -r '.[] | select(.class | test("hyprland")) | .address' | head -n1)
    [ -n "$a" ] && { hc dispatch "hl.dsp.window.close({ window = \"address:$a\" })" >/dev/null; sleep 0.5; break; }
    sleep 0.2
done

cap=${UQ_CAPTURE_DIR:-}; grabber=""
shot() { :; }
if [ -n "$cap" ] && command -v grim >/dev/null 2>&1; then
    mkdir -p "$cap/frames"
    shot() { env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" grim "$@" 2>/dev/null; }
    (n=0; while :; do shot "$cap/frames/$(printf %04d $n).png"; n=$((n + 1)); sleep 0.25; done) &
    grabber=$!; pids+=($!)
fi
still() { [ -n "$cap" ] && shot "$cap/clip-$1.png"; }

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
popup() { hc layers -j | grep -o '"namespace": *"uq-clip"' | wc -l; }
wait_popup() { for _ in $(seq 1 40); do [ "$(popup)" -eq "$1" ] && return 0; sleep 0.2; done; return 1; }
pasted() { env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" wl-paste --no-newline 2>/dev/null; }
previews() { clip list --json | jq -r '.[].preview' | paste -sd'|'; }
bad=0
check() { echo "FAIL: $*"; bad=1; }
copy() { printf '%s' "$1" | env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" wl-copy; sleep 0.7; }

# --- copy some things ---------------------------------------------------------------------------
copy "the first thing I copied"
copy "second: a longer line of text that should be shown on a single row of the history"
env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" grim "$tmp/bar.png" && env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" wl-copy --type image/png <"$tmp/bar.png"; sleep 0.8
[ "$(clip list --json | jq length)" = 3 ] || check "expected 3 entries, got: $(previews)"
[ "$(clip list --kind image --json | jq length)" = 1 ] || check "the image was not recorded"
for _ in $(seq 1 30); do grep -q 'SLOT' "$tmp/qs.log" && break; sleep 0.25; done
grep -q 'SLOT .* true' "$tmp/qs.log" || check "the Clipboard island is not shown on the bar"
still 1-bar

# --- the real shortcut opens the popup ------------------------------------------------------------
send "mod super down" "mod shift down" "key v" "mod shift up" "mod super up" "sleep 900"; drain
wait_popup 1 || check "SUPER+SHIFT+V did not open the history"
still 2-popup
send "type first" "sleep 600"; drain
still 3-filtered
send "key Return" "sleep 900"; drain
wait_popup 0 || check "the popup stayed open after Enter"
[ "$(pasted)" = "the first thing I copied" ] || check "Enter did not put the entry back (clipboard: $(pasted))"
[ "$(clip list --json | jq -r '.[0].preview')" = "the first thing I copied" ] || check "the copied-back entry is not at the top"
still 4-copied-back

# --- pin and forget, through the IPC route ---------------------------------------------------------
env "${env_all[@]:1}" timeout 10 qs ipc -p "$tmp/harness/shell.qml" call uq clip >/dev/null 2>&1
wait_popup 1 || check "qs ipc call uq clip did not open the history"
send "type second" "sleep 500" "mod ctrl down" "key p" "mod ctrl up" "sleep 900"; drain
[ "$(clip list --json | jq -r '[.[] | select(.pinned)] | length')" = 1 ] || check "Ctrl+P did not pin"
[ "$(clip list --json | jq -r '.[] | select(.pinned) | .preview' | cut -c1-6)" = "second" ] || check "the wrong entry was pinned"
still 5-pinned
send "key Delete" "sleep 900"; drain
[ "$(clip list --json | jq length)" = 2 ] || check "Delete did not forget the entry: $(previews)"
send "key Escape" "sleep 700"; drain
wait_popup 0 || check "Escape did not close the popup"
still 6-closed

grep -qE 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|Cannot read property|is not a function|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? ' "$tmp/qs.log" &&
    { grep -E 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|Cannot read property|is not a function|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? ' "$tmp/qs.log" | head -n 10; check "QML errors"; }

if [ -n "$grabber" ]; then
    sleep 0.5; kill "$grabber" 2>/dev/null
    command -v ffmpeg >/dev/null 2>&1 && ffmpeg -loglevel error -y -framerate 4 -i "$cap/frames/%04d.png" \
        -vf "scale=960:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse" "$cap/clip.gif" 2>&1 | tail -n 3
    rm -rf "$cap/frames"; ls -l "$cap"
fi
echo "--- qs.log (tail)"; tail -n 12 "$tmp/qs.log"
[ "$bad" = 0 ] && echo "clip-live: all checks held"
exit "$bad"
