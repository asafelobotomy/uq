#!/usr/bin/env bash
# Moving a window with the pointer on a real (headless) Hyprland with the real shell, in the scrolling
# layout: what the shell does while the window is held. Super+drag is played through a virtual
# keyboard and pointer (tests/live/tools/uqinput).
#
#   scripts/drag-live.sh [--scenario half-pair|full-edge|keys|restore] [--unguarded]
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
unguarded=0 scenario=half-pair
while [ $# -gt 0 ]; do
    case "$1" in
        --unguarded) unguarded=1 ;;
        --scenario) scenario=${2:?--scenario needs a name}; shift ;;
        *) echo "unknown option $1" >&2; exit 2 ;;
    esac
    shift
done

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

# Hyprland's own first-run notice is a floating window over the middle of the screen: close it.
for _ in $(seq 1 15); do
    a=$(hc clients -j | jq -r '.[] | select(.class | test("hyprland")) | .address' | head -n1)
    [ -n "$a" ] && { hc dispatch "hl.dsp.window.close({ window = \"address:$a\" })" >/dev/null; sleep 0.5; break; }
    sleep 0.2
done

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

case "$scenario" in
    half-pair)
        # Two Half windows share the screen. Take w2 and put it down where it was.
        open_window w1; settle 1.5
        open_window w2; settle 1.5
        width w1 half; width w2 half; settle 2
        grab=w2; grab_dx=0.5 ;;
    full-edge)
        # Two Full windows. Take w2 by its right side: lifted, it hangs over the screen's edge.
        open_window w1; settle 1.5
        open_window w2; settle 2
        grab=w2; grab_dx=0.93 ;;
    keys)
        # Two Half windows; the keyboard actions that do what the mouse does (shortcuts in
        # Settings): move the column, Full and back, wider and narrower.
        open_window w1; settle 1.5
        open_window w2; settle 1.5
        width w1 half; width w2 half; settle 2 ;;
    restore)
        # A window minimised to its tab, then chosen from the bar: it must end up focused.
        open_window w1; settle 1.5
        open_window w2; settle 2 ;;
    *) fail "unknown scenario $scenario" ;;
esac
echo "--- before"
snap before

if [ "$scenario" = keys ]; then
    # The same Lua the shortcuts in config/hypr/base.lua run, sent as dispatches.
    x() { hc clients -j | jq -r --arg t "$1" '.[] | select(.title == $t) | .at[0]'; }
    w() { hc clients -j | jq -r --arg t "$1" '.[] | select(.title == $t) | .size[0]'; }
    fs() { hc clients -j | jq -r --arg t "$1" '.[] | select(.title == $t) | .fullscreen'; }
    act() { hc dispatch "$1" >/dev/null; settle 1; }
    bad=0
    w2x=$(x w2); w1x=$(x w1)
    [ "$w1x" -lt "$w2x" ] || { echo "FAIL: w1 should start left of w2 ($w1x, $w2x)"; bad=1; }
    act 'hl.dsp.layout("swapcol l")'
    [ "$(x w2)" -lt "$(x w1)" ] || { echo "FAIL: swapcol l did not move w2's column left"; bad=1; }
    act 'hl.dsp.layout("swapcol r")'
    [ "$(x w1)" -lt "$(x w2)" ] || { echo "FAIL: swapcol r did not move it back"; bad=1; }
    before_w=$(w w2)
    act 'hl.dsp.layout("colresize +0.1")'
    [ "$(w w2)" -gt "$before_w" ] || { echo "FAIL: colresize +0.1 did not widen ($before_w -> $(w w2))"; bad=1; }
    act 'hl.dsp.layout("colresize -0.1")'
    act 'hl.dsp.window.fullscreen({ mode = "maximized" })'
    [ "$(fs w2)" = 1 ] || { echo "FAIL: the Full shortcut did not make w2 Full"; bad=1; }
    act 'hl.dsp.window.fullscreen({ mode = "maximized" })'
    [ "$(fs w2)" = 0 ] || { echo "FAIL: the Full shortcut again did not make w2 Half"; bad=1; }
    echo "--- after"; snap after
    [ "$bad" = 0 ] && echo "drag-live keys: all checks held"
    exit "$bad"
fi

if [ "$scenario" = restore ]; then
    # What the bar does: Minimise w1 (focus is on w2), then Restore it from its tab.
    active() { hc activewindow -j | jq -r '.title // ""'; }
    d() { bridge_call "{\"v\":1,\"id\":1,\"call\":\"Dispatch\",\"args\":$1}" >/dev/null; }
    a1=$(addr_of w1); bad=0
    d "[\"minimise\",{\"address\":\"$a1\",\"from_ws\":1}]"; settle 2
    [ "$(active)" = w2 ] || { echo "FAIL: w2 should be focused once w1 is minimised (got $(active))"; bad=1; }
    d "[\"restore\",{\"address\":\"$a1\",\"to_ws\":1}]"; settle 3
    echo "--- after"; snap after
    [ "$(active)" = w1 ] || { echo "FAIL: the restored window lost focus (active is '$(active)', not w1)"; bad=1; }
    echo "--- focus events after the restore"; grep activewindow "$tmp/events.log" | tail -n 6
    [ "$bad" = 0 ] && echo "drag-live restore: all checks held"
    exit "$bad"
fi

# Drive the pointer from a fifo, so the state can be read between steps.
mkfifo "$tmp/in"
env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" "$tmp/uqinput" 1280 720 <"$tmp/in" >"$tmp/uqinput.out" 2>&1 &
pids+=($!)
exec 7>"$tmp/in"
for _ in $(seq 1 50); do grep -q ready "$tmp/uqinput.out" && break; sleep 0.1; done
grep -q ready "$tmp/uqinput.out" || fail "uqinput: $(cat "$tmp/uqinput.out")"
sent=0
send() { sent=$((sent + $#)); printf '%s\n' "$@" >&7; }
# Wait until uqinput has done everything sent so far (it answers "ok" to each command).
drain() {
    for _ in $(seq 1 300); do
        [ "$(grep -c '^ok' "$tmp/uqinput.out")" -ge "$sent" ] && return 0
        sleep 0.1
    done
    fail "uqinput didn't finish its commands ($(tail -n 3 "$tmp/uqinput.out"))"
}

read -r cx cy < <(hc clients -j | jq -r --arg t "$grab" --argjson f "$grab_dx" \
    '.[] | select(.title==$t) | "\((.at[0] + .size[0] * $f)|floor) \((.at[1] + .size[1]/2)|floor)"')
echo "grab $grab at $cx,$cy"
t0=$(date +%s.%N)
send "move $cx $cy" "sleep 300" "mod super down" "sleep 100" "button 272 down" "sleep 100"
if [ "$scenario" = half-pair ]; then
    for i in 1 2 3 4 5 6 7 8 9 10 11 12; do send "move $((cx + i * 20)) $((cy + i * 10))" "sleep 40"; done
    send "sleep 1200"
    drain; echo "--- while held"; snap held | tee "$tmp/held.txt"
    for i in 11 10 9 8 7 6 5 4 3 2 1 0; do send "move $((cx + i * 20)) $((cy + i * 10))" "sleep 40"; done
else
    for i in 1 2 3 4 5 6 7 8 9 10; do send "move $((cx - i * 8)) $((cy + i * 4))" "sleep 40"; done
    send "sleep 1200"
    drain; echo "--- while held"; snap held | tee "$tmp/held.txt"
fi
send "sleep 300" "button 272 up" "sleep 100" "mod super up"
drain; settle 2
echo "--- after"
snap after

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
echo "RESULT scenario=$scenario mode=$([ "$unguarded" = 1 ] && echo unguarded || echo guarded) begin_end_seen=$([ -n "$begin" ] && [ -n "$end" ] && echo yes || echo no) focus_events_in_hold=$focus_changes"
echo "--- qs.log (tail)"; tail -n 8 "$tmp/qs.log"

# What a drag must leave behind (guarded runs only; --unguarded is for looking at the difference).
[ "$unguarded" = 1 ] && exit 0
bad=0
win() { hc clients -j | jq -r --arg t "$1" --arg k "$2" '.[] | select(.title == $t) | .[$k] | tostring'; }
[ -n "$begin" ] && [ -n "$end" ] || { echo "FAIL: no begin/end events from the mouse hooks"; bad=1; }
for w in w1 w2; do
    [ "$(win $w floating)" = false ] || { echo "FAIL: $w is still floating after the drop"; bad=1; }
done
case "$scenario" in
    half-pair)
        # The neighbour must not be made Full under the pointer, and both are Half columns after.
        for w in w1 w2; do
            [ "$(win $w fullscreen)" = 0 ] || { echo "FAIL: $w ended up Full (fullscreen=$(win $w fullscreen))"; bad=1; }
        done
        [ "$(awk '/^held w1 /{print $NF}' "$tmp/held.txt")" = fs=0 ] || { echo "FAIL: w1 was made Full while w2 was held"; bad=1; } ;;
    full-edge)
        # A Full window that was dragged comes back Full, and nothing else changed.
        [ "$(win w2 fullscreen)" = 1 ] || { echo "FAIL: w2 was Full before the drag and is not after it"; bad=1; }
        [ "$(win w1 fullscreen)" = 1 ] || { echo "FAIL: w1 lost Full"; bad=1; }
        # Held and hanging over the screen's edge, it is left floating where the pointer put it.
        grep -q '^held w2 .* floating=true' "$tmp/held.txt" || { echo "FAIL: w2 was not left floating while held"; bad=1; } ;;
esac
# Focus: moving the held window must not hop focus to others.
[ "$focus_changes" -le 3 ] || { echo "FAIL: focus changed $focus_changes times during the hold"; bad=1; }
[ "$bad" = 0 ] && echo "drag-live $scenario: all checks held"
exit "$bad"
