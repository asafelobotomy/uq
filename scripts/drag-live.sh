#!/usr/bin/env bash
# Moving a window with the pointer on a real (headless) Hyprland with the real shell, in the scrolling
# layout: what the shell does while the window is held. Super+drag is played through a virtual
# keyboard and pointer (tests/live/tools/uqinput).
#
#   scripts/drag-live.sh [--unguarded]
#
# It starts Hyprland on uQ's base.lua and the shell, opens windows, drags one and prints what
# happened between the drag's start and end events: the focus changes, and where the dragged window
# and the others were. --unguarded runs the shell with WindowDrag turned off (what the shell did
# before it existed), for comparison.
#
# Needs the same as scripts/smoke-shell.sh (a DRM card, seatd in a container), plus foot, jq,
# socat, gcc, wayland and libxkbcommon. Exit 0 when it ran and the checks held; 77 when this
# environment can't run it; 1 when a check failed.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
unguarded=0
[ "${1:-}" = --unguarded ] && unguarded=1

skip() { echo "SKIP: $*"; exit 77; }
for bin in Hyprland hyprctl qs dbus-daemon jq foot socat gcc wayland-scanner; do
    command -v "$bin" >/dev/null 2>&1 || skip "$bin is not installed"
done
ls /dev/dri/card* >/dev/null 2>&1 || skip "no DRM device"
uqctl=${UQCTL:-$root/target/debug/uqctl}
[ -x "$uqctl" ] || skip "no uqctl binary"

tmp=$(mktemp -d /tmp/uqd.XXXXXX)
rt="$tmp/rt"; mkdir -m 700 "$rt"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; wait 2>/dev/null; rm -rf "$tmp"; }
trap cleanup EXIT
fail() { echo "FAIL: $*"; for f in hypr qs; do [ -f "$tmp/$f.log" ] && { echo "--- $f.log"; tail -n 30 "$tmp/$f.log"; }; done; exit 1; }

"$root/tests/live/tools/build-uqinput.sh" "$tmp/uqinput" || skip "uqinput didn't build"

dbus-daemon --session --address="unix:path=$tmp/bus" --nofork --nopidfile >"$tmp/dbus.log" 2>&1 &
pids+=($!)

shell_dir=$root/shell
if [ "$unguarded" = 1 ]; then
    shell_dir=$tmp/shell
    cp -r "$root/shell" "$shell_dir"
    # The hold never starts: the handlers see active = false throughout.
    sed -i 's/root.active = true;/root.active = false;/' "$shell_dir/services/WindowDrag.qml"
    grep -q 'root.active = false;' "$shell_dir/services/WindowDrag.qml" || fail "couldn't turn WindowDrag off"
fi

cat >"$tmp/hypr.lua" <<LUA
require([[$root/config/hypr/base.lua]])
hl.monitor({ output = "", mode = "1280x720", position = "auto", scale = 1 })
hl.config({ general = { layout = "scrolling", gaps_in = 5, gaps_out = 12, border_size = 2 },
            misc = { disable_hyprland_logo = true, disable_splash_rendering = true } })
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
echo "hyprland: $(hc version -j | jq -r '.tag // .version')"
hc configerrors | grep -v '^\s*$' | sed 's/^/config error: /'

env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" HYPRLAND_INSTANCE_SIGNATURE="$sig" \
    DBUS_SESSION_BUS_ADDRESS="unix:path=$tmp/bus" PATH="$(dirname "$uqctl"):$PATH" XDG_STATE_HOME="$tmp/state" \
    QT_QPA_PLATFORM=wayland LC_ALL=C.UTF-8 qs -p "$shell_dir" --no-duplicate >"$tmp/qs.log" 2>&1 &
pids+=($!)
for _ in $(seq 1 50); do hc layers -j | grep -q uq-bar && break; sleep 0.2; done
hc layers -j | grep -q uq-bar || fail "the bar never appeared"

# Everything Hyprland says, with when.
socat -u "UNIX-CONNECT:$rt/hypr/$sig/.socket2.sock" - 2>/dev/null | while IFS= read -r l; do
    printf '%s %s\n' "$(date +%s.%N)" "$l"; done >"$tmp/events.log" &
pids+=($!)

open_window() { # title
    env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" LIBGL_ALWAYS_SOFTWARE=1 foot -T "$1" sleep 600 >"$tmp/foot-$1.log" 2>&1 &
    pids+=($!)
    for _ in $(seq 1 50); do hc clients -j | jq -e --arg t "$1" 'any(.[]; .title == $t)' >/dev/null && return 0; sleep 0.2; done
    fail "window $1 never opened"
}
snap() { # label: one line per window
    hc clients -j | jq -r --arg l "$1" '.[] | "\($l) \(.title) at=\(.at|join(",")) size=\(.size|join(",")) floating=\(.floating) ws=\(.workspace.id) fs=\(.fullscreen)"' | sort
}
settle() { sleep "${1:-2}"; }

bridge_call() {
    { printf '%s\n' "$1"; sleep 1.5; } |
        env XDG_RUNTIME_DIR="$rt" HYPRLAND_INSTANCE_SIGNATURE="$sig" DBUS_SESSION_BUS_ADDRESS="unix:path=$tmp/bus" "$uqctl" bridge 2>/dev/null
}
addr_of() { hc clients -j | jq -r --arg t "$1" '.[] | select(.title == $t) | .address'; }
width() { # title full|half
    bridge_call "{\"v\":1,\"id\":1,\"call\":\"Dispatch\",\"args\":[\"column_width\",{\"address\":\"$(addr_of "$1")\",\"width\":\"$2\"}]}" >/dev/null
}

open_window w1; settle 1.5
open_window w2; settle 1.5
open_window w3; settle 2
width w1 half; width w2 half; settle 2
echo "--- before"
snap before

# Drive the pointer from a fifo, so the state can be read between steps.
mkfifo "$tmp/in"
env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" "$tmp/uqinput" 1280 720 <"$tmp/in" >"$tmp/uqinput.out" 2>&1 &
pids+=($!)
exec 7>"$tmp/in"
for _ in $(seq 1 50); do grep -q ready "$tmp/uqinput.out" && break; sleep 0.1; done
grep -q ready "$tmp/uqinput.out" || fail "uqinput: $(cat "$tmp/uqinput.out")"
send() { printf '%s\n' "$@" >&7; }

# The centre of w2's frame.
read -r cx cy < <(hc clients -j | jq -r '.[] | select(.title=="w2") | "\(.at[0] + (.size[0]/2|floor)) \(.at[1] + (.size[1]/2|floor))"')
echo "grab w2 at $cx,$cy"
t0=$(date +%s.%N)
send "move $cx $cy" "sleep 300" "mod super down" "sleep 100" "button 272 down" "sleep 100"
for i in 1 2 3 4 5 6 7 8 9 10 11 12; do send "move $((cx + i * 20)) $((cy + i * 10))" "sleep 40"; done
send "sleep 1200"
echo "--- while held"
snap held
send "move $((cx + 260)) $((cy + 130))" "sleep 300" "button 272 up" "sleep 100" "mod super up"
settle 3
echo "--- after"
snap after
t1=$(date +%s.%N)

echo "--- events between the first custom event and the last"
begin=$(awk '/custom>>uq-mouse:begin/{print $1; exit}' "$tmp/events.log")
end=$(awk '/custom>>uq-mouse:end/{t=$1} END{print t}' "$tmp/events.log")
echo "begin=${begin:-none} end=${end:-none}"
if [ -n "$begin" ] && [ -n "$end" ]; then
    awk -v b="$begin" -v e="$end" '$1 >= b && $1 <= e + 0.5 {print}' "$tmp/events.log" | sed 's/^\([0-9]*\.[0-9]\{3\}\)[0-9]* /\1 /'
else
    echo "no begin/end custom events; all events since the grab:"
    awk -v b="$t0" '$1 >= b {print}' "$tmp/events.log" | head -n 80
fi

in_hold() { awk -v b="${begin:-0}" -v e="${end:-0}" -v n="$1" '$1 >= b && $1 <= e && $2 ~ n' "$tmp/events.log" | wc -l; }
focus_changes=$(in_hold '^activewindowv2>>')
echo "RESULT mode=$([ "$unguarded" = 1 ] && echo unguarded || echo guarded) begin_end_seen=$([ -n "$begin" ] && [ -n "$end" ] && echo yes || echo no) focus_events_in_hold=$focus_changes"
echo "--- qs.log (tail)"; tail -n 8 "$tmp/qs.log"
exit 0
