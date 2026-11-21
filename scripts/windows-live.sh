#!/usr/bin/env bash
# Window handling end to end, on a real (headless) Hyprland with the real shell, in the scrolling
# layout, driven by a virtual keyboard and pointer (tests/live/tools/uqinput), with timings.
#
#   scripts/windows-live.sh
#
# One session walks through what people do with windows and checks what each step leaves behind:
#   open      windows open; a lone window is Full, the next one gets a page of its own
#   half      both made Half: each keeps its own page, and no grip shows at the screen's edge
#   tabmove   a tab dragged along the bar: the window's column follows
#   tabdrop   a tab dragged down onto the screen: its window fills the empty half, focused
#   grip      the gap between that pair dragged with the pointer: only the gap moves
#   superdrag Super+drag the left window to the right half: they swap, nothing gets stacked
#   minimise  minimised to its tab, then restored by clicking the tab: back at the end, focused
#   float     a floating window dragged half off the screen stays where it was put
#   fullscreen real fullscreen hides the resize grips; leaving it brings them back
#   workspace a window moved to workspace 2 and back, and to a special workspace and back
#   close     windows closed: tabs and pages follow
# and measures each step: how long until the result shows (latency), the shell's frames (Qt's
# QSG_RENDER_TIMING: count, median, 95th percentile, worst) and the CPU the shell and Hyprland used.
# Rendering is software (llvmpipe), so frame times only compare with other runs on the same kind of
# machine. With UQ_CAPTURE_DIR set (CI does) it keeps a screenshot after each step and writes
# windows-perf.md there.
#
# The shell runs from a copy with one addition: a probe in the Work island that logs where each
# tab is, so the pointer can find them. Needs the same as scripts/drag-live.sh. Exit 0 when it ran
# and the checks held; 77 when this environment can't run it; 1 when a check failed.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)

skip() { echo "SKIP: $*"; exit 77; }
for bin in Hyprland hyprctl qs dbus-daemon jq foot socat gcc wayland-scanner python3; do
    command -v "$bin" >/dev/null 2>&1 || skip "$bin is not installed"
done
ls /dev/dri/card* >/dev/null 2>&1 || skip "no DRM device"
uqctl=${UQCTL:-$root/target/debug/uqctl}
[ -x "$uqctl" ] || skip "no uqctl binary"

tmp=$(mktemp -d /tmp/uqw.XXXXXX)
rt="$tmp/rt"; mkdir -m 700 "$rt"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; wait 2>/dev/null; rm -rf "$tmp"; }
trap cleanup EXIT
fail() { echo "FAIL: $*"; for f in hypr qs; do [ -f "$tmp/$f.log" ] && { echo "--- $f.log"; grep -v 'frame rendered\|qt.scenegraph' "$tmp/$f.log" | tail -n 40; }; done; exit 1; }
bad=0
check() { echo "FAIL: $*"; bad=1; }
cap=${UQ_CAPTURE_DIR:-}
[ -n "$cap" ] && mkdir -p "$cap"

"$root/tests/live/tools/build-uqinput.sh" "$tmp/uqinput" || skip "uqinput didn't build"

# The shell, with the tab probe.
cp -r "$root/shell" "$tmp/shell"
python3 -I - "$tmp/shell/bar/WorkIsland.qml" <<'PY' || fail "couldn't add the tab probe"
import sys
p = sys.argv[1]
s = open(p).read().rstrip()
assert s.endswith("}")
probe = '''
    // windows-live.sh: where each tab is, in the bar window's coordinates.
    property string probeLast: ""
    onTabInfoChanged: Qt.callLater(probeTimer.probe)
    onTabKeysChanged: Qt.callLater(probeTimer.probe)
    Timer {
        id: probeTimer
        interval: 150; running: true; repeat: true
        onTriggered: probe()
        function probe() {
            var out = [];
            for (var k = 0; k < tabsRep.count; k++) {
                var it = tabsRep.itemAt(k);
                if (!it)
                    continue;
                var p = it.mapToItem(null, 0, 0);
                out.push({ a: it.win.address, t: it.name, x: Math.round(p.x), y: Math.round(p.y),
                           w: Math.round(it.width), h: Math.round(it.height),
                           m: it.win.minimised === true, f: it.win.focused === true });
            }
            var s = JSON.stringify(out);
            if (s !== root.probeLast) {
                root.probeLast = s;
                console.log("UQPROBE " + Date.now() + " " + s);
            }
        }
    }
}
'''
open(p, "w").write(s[:-1] + probe)
PY

dbus-daemon --session --address="unix:path=$tmp/bus" --nofork --nopidfile >"$tmp/dbus.log" 2>&1 &
pids+=($!)

cat >"$tmp/hypr.lua" <<LUA
require([[$root/config/hypr/base.lua]])
hl.monitor({ output = "", mode = "1280x720", position = "auto", scale = 1 })
hl.config({ general = { layout = "scrolling", gaps_in = 5, gaps_out = 12, border_size = 2 },
            misc = { disable_hyprland_logo = true, disable_splash_rendering = true } })
LUA
env XDG_RUNTIME_DIR="$rt" LIBGL_ALWAYS_SOFTWARE=1 UQ_DEV_SESSION=1 Hyprland -c "$tmp/hypr.lua" >"$tmp/hypr.log" 2>&1 &
hypr_pid=$!; pids+=($hypr_pid)
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
    QT_QPA_PLATFORM=wayland LC_ALL=C.UTF-8 QSG_RENDER_TIMING=1 \
    qs -p "$tmp/shell" --no-duplicate >"$tmp/qs.log" 2>&1 &
qs_pid=$!; pids+=($qs_pid)
for _ in $(seq 1 50); do hc layers -j | grep -q uq-bar && break; sleep 0.2; done
hc layers -j | grep -q uq-bar || fail "the bar never appeared"

for _ in $(seq 1 15); do
    a=$(hc clients -j | jq -r '.[] | select(.class | test("hyprland")) | .address' | head -n1)
    [ -n "$a" ] && { hc dispatch "hl.dsp.window.close({ window = \"address:$a\" })" >/dev/null; sleep 0.5; break; }
    sleep 0.2
done

socat -u "UNIX-CONNECT:$rt/hypr/$sig/.socket2.sock" - 2>/dev/null | while IFS= read -r l; do
    printf '%s %s\n' "$(date +%s.%N)" "$l"; done >"$tmp/events.log" &
pids+=($!)

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
        sleep 0.05
    done
    fail "uqinput didn't finish its commands ($(tail -n 3 "$tmp/uqinput.out"))"
}
send "move 640 400"; drain

# ---- helpers ----------------------------------------------------------------------------------
now_ms() { echo $(( $(date +%s%N) / 1000000 )); }
clients() { hc clients -j; }
addr_of() { clients | jq -r --arg t "$1" '.[] | select(.title == $t) | .address'; }
win() { clients | jq -r --arg t "$1" --arg k "$2" '.[] | select(.title == $t) | .[$k] | tostring'; }
wx() { clients | jq -r --arg t "$1" '.[] | select(.title == $t) | .at[0]'; }
wwidth() { clients | jq -r --arg t "$1" '.[] | select(.title == $t) | .size[0]'; }
active() { hc activewindow -j | jq -r '.title // ""'; }
snap() { clients | jq -r --arg l "$1" '.[] | "\($l) \(.title) at=\(.at|join(",")) size=\(.size|join(",")) floating=\(.floating) ws=\(.workspace.name) fs=\(.fullscreen) hidden=\(.hidden)"' | sort; }
bridge_call() {
    { printf '%s\n' "$1"; sleep 1; } |
        env XDG_RUNTIME_DIR="$rt" HYPRLAND_INSTANCE_SIGNATURE="$sig" DBUS_SESSION_BUS_ADDRESS="unix:path=$tmp/bus" "$uqctl" bridge 2>/dev/null
}
dispatch() { bridge_call "{\"v\":1,\"id\":1,\"call\":\"Dispatch\",\"args\":$1}" >/dev/null; }
# The same, not waiting for the answer: for timing what follows a dispatch (the time includes
# starting a `uqctl bridge`, which the shell's long-running bridge doesn't pay).
dispatch_now() {
    ( { printf '%s\n' "{\"v\":1,\"id\":1,\"call\":\"Dispatch\",\"args\":$1}"; sleep 2; } |
        env XDG_RUNTIME_DIR="$rt" HYPRLAND_INSTANCE_SIGNATURE="$sig" DBUS_SESSION_BUS_ADDRESS="unix:path=$tmp/bus" "$uqctl" bridge >/dev/null 2>&1 ) &
}
# Wait for a condition (shell code, run here), polling every 25 ms; prints the ms it took, or "timeout".
wait_ms() { # timeout_s code
    local limit=$1 code=$2
    local t0; t0=$(now_ms)
    while :; do
        if eval "$code" >/dev/null 2>&1; then echo $(( $(now_ms) - t0 )); return 0; fi
        [ $(( $(now_ms) - t0 )) -gt $((limit * 1000)) ] && { echo timeout; return 1; }
        sleep 0.025
    done
}
# Where the tabs are: the probe's last line, with the bar layer's corner added.
bar_xy() { hc layers -j | jq -r '.. | objects | select(.namespace? == "uq-bar") | "\(.x) \(.y)"' | head -n1; }
tabs() { grep 'UQPROBE ' "$tmp/qs.log" | tail -n1 | sed 's/.*UQPROBE [0-9]* //'; }
# When Hyprland sent the first event matching a pattern since the step began (ms since the epoch).
event_ms() { awk -v t="$phase_t0" -v p="$1" '$1 * 1000 >= t && $2 ~ p { printf "%d\n", $1 * 1000; exit }' "$tmp/events.log"; }
# When the tabs first met a jq condition at or after a time (ms since the epoch).
probe_ms() { # since jq-condition
    grep 'UQPROBE ' "$tmp/qs.log" | sed 's/.*UQPROBE //' | while read -r t j; do
        [ "$t" -ge "$1" ] && echo "$j" | jq -e "$2" >/dev/null 2>&1 && { echo "$t"; break; }
    done
}
# The shell's part of a latency: from Hyprland's event to the tabs showing it.
shell_part() { # event-pattern jq-condition label
    local e p; e=$(event_ms "$1"); [ -n "$e" ] || return 0
    p=$(probe_ms "$e" "$2"); [ -n "$p" ] || return 0
    echo "shell part ($3): $((p - e)) ms after Hyprland's event"
    phase_what="$phase_what; the bar $((p - e)) ms after Hyprland's event"
}
tab_of() { # title -> "cx cy w" (centre, screen coordinates)
    local bx by; read -r bx by < <(bar_xy)
    tabs | jq -r --arg t "$1" --argjson bx "$bx" --argjson by "$by" \
        '.[] | select(.t == $t) | "\((.x + .w/2 + $bx)|floor) \((.y + .h/2 + $by)|floor) \(.w)"' | head -n1
}
tab_flag() { tabs | jq -r --arg t "$1" --arg k "$2" '.[] | select(.t == $t) | .[$k] | tostring' | head -n1; }
tab_order() { tabs | jq -r 'sort_by(.x) | map(.t) | join(" | ")'; }
col_order() { clients | jq -r '[.[] | select(.workspace.id == 1 and .floating == false)] | sort_by(.at[0]) | map(.title) | join(" | ")'; }
# A window on the workspace shown must be on the screen or off it, not cut by its edge. A strip of
# up to 8 px (a neighbouring page's frame, which the gaps don't cover) is counted as a sliver.
slivers=0
edge_ok() { # title context
    local x w vis
    x=$(wx "$1"); w=$(wwidth "$1")
    [ -n "$x" ] || return 0
    vis=$(( (x + w > 1280 ? 1280 : x + w) - (x < 0 ? 0 : x) ))
    [ "$vis" -le 0 ] || [ "$vis" -ge "$w" ] && return 0
    if [ "$vis" -le 8 ]; then echo "sliver: $vis px of $1 shows at the screen's edge $2"; slivers=$((slivers + 1)); return 0; fi
    check "$1 is cut by the screen's edge $2 ($vis of $w px show)"
}
on_screen() { # title: fully inside the 1280x720 screen
    clients | jq -e --arg t "$1" '.[] | select(.title == $t) | (.at[0] >= 0 and .at[1] >= 0 and .at[0] + .size[0] <= 1280 and .at[1] + .size[1] <= 720)'
}
open_window() {
    env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" LIBGL_ALWAYS_SOFTWARE=1 foot -T "$1" sleep 900 >"$tmp/foot-$1.log" 2>&1 &
    pids+=($!)
}
shot() { [ -n "$cap" ] && command -v grim >/dev/null 2>&1 && env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" grim "$cap/windows-$1.png" 2>/dev/null; return 0; }
settle() { sleep "${1:-1.5}"; }

# ---- measuring --------------------------------------------------------------------------------
ticks() { awk '{print $14 + $15}' "/proc/$1/stat" 2>/dev/null || echo 0; }
hz=$(getconf CLK_TCK)
phase_name="" phase_t0=0 phase_line=0 phase_qs=0 phase_hy=0 phase_lat=""
printf '| Step | Latency (ms) | What the latency is | Frames | p50 (ms) | p95 (ms) | Worst (ms) | Shell CPU %% | Hyprland CPU %% |\n|---|---|---|---|---|---|---|---|---|\n' >"$tmp/perf.md"
begin() { # name
    phase_name=$1 phase_t0=$(now_ms) phase_line=$(wc -l <"$tmp/qs.log")
    phase_qs=$(ticks "$qs_pid") phase_hy=$(ticks "$hypr_pid") phase_lat="" phase_what=""
    echo "=========== $1"
}
latency() { phase_lat=$1 phase_what=$2; echo "latency: $1 ms ($2)"; }
finish() {
    local dt=$(( $(now_ms) - phase_t0 ))
    local qs=$(( $(ticks "$qs_pid") - phase_qs )) hy=$(( $(ticks "$hypr_pid") - phase_hy ))
    local stats
    stats=$(tail -n +"$((phase_line + 1))" "$tmp/qs.log" | python3 -I -c '
import re, sys
v = sorted(float(m.group(1)) for l in sys.stdin if "frame rendered in" in l
           for m in [re.search(r"([0-9]+(?:\.[0-9]+)?)\s*ms", l)] if m)
q = lambda f: v[min(len(v) - 1, int(f * len(v)))] if v else 0
print(len(v), "%.1f" % q(0.5), "%.1f" % q(0.95), "%.1f" % (v[-1] if v else 0))')
    local n p50 p95 mx; read -r n p50 p95 mx <<<"$stats"
    local qc hc_ ; qc=$(awk -v t="$qs" -v h="$hz" -v d="$dt" 'BEGIN{printf "%.1f", (d > 0 ? 100 * t / h / (d / 1000) : 0)}')
    hc_=$(awk -v t="$hy" -v h="$hz" -v d="$dt" 'BEGIN{printf "%.1f", (d > 0 ? 100 * t / h / (d / 1000) : 0)}')
    echo "PERF $phase_name dur=${dt}ms latency=${phase_lat:--}ms frames=$n p50=$p50 p95=$p95 max=$mx shell_cpu=$qc% hypr_cpu=$hc_%"
    printf '| %s | %s | %s | %s | %s | %s | %s | %s | %s |\n' "$phase_name" "${phase_lat:--}" "${phase_what:--}" \
        "$n" "$p50" "$p95" "$mx" "$qc" "$hc_" >>"$tmp/perf.md"
    snap "  $phase_name"
    shot "$phase_name"
}

# ---- idle -------------------------------------------------------------------------------------
settle 2
begin idle
sleep 3
finish

# ---- open -------------------------------------------------------------------------------------
begin open
t0=$(now_ms); open_window w1
wait_ms 10 '[ -n "$(addr_of w1)" ]' >/dev/null || fail "w1 never opened"
map=$(( $(now_ms) - t0 ))
tabms=$(wait_ms 5 '[ -n "$(tab_of w1)" ]')
tabms=$([ "$tabms" = timeout ] && echo timeout || echo $(( map + tabms )))
[ "$tabms" = timeout ] && check "w1 never got a tab"
latency "$tabms" "launch until its tab is on the bar (window mapped at ${map} ms)"
shell_part "^openwindow>>" 'any(.[]; .t == "w1")' open
settle
# ColumnPolicy: a lone window is Full.
[ "$(wwidth w1)" -ge 1200 ] || check "a lone window should be Full (w1 is $(wwidth w1) wide)"
open_window w2
wait_ms 10 '[ -n "$(addr_of w2)" ]' >/dev/null || fail "w2 never opened"
settle
[ "$(wwidth w2)" -ge 1200 ] || check "a window opened from a Full one should be Full (w2 is $(wwidth w2) wide)"
on_screen w2 >/dev/null || check "the new window w2 should fill the screen"
[ "$(active)" = w2 ] || check "the new window should have focus (active is '$(active)')"
finish

# ---- half -------------------------------------------------------------------------------------
# Both made Half: each keeps its own screen page (the pages remember where they begin), so the
# focused one shows on the left and the rest of the screen is empty desktop.
begin half
dispatch "[\"column_width\",{\"address\":\"$(addr_of w1)\",\"width\":\"half\"}]"
dispatch_now "[\"column_width\",{\"address\":\"$(addr_of w2)\",\"width\":\"half\"}]"
ms=$(wait_ms 5 '[ "$(wwidth w2)" -lt 700 ] && on_screen w2')
latency "$ms" "Half dispatch until the window is Half on screen"
settle
on_screen w2 >/dev/null || check "the focused Half window w2 should be on screen"
[ "$(wwidth w2)" -lt 700 ] || check "w2 should be Half ($(wwidth w2) wide)"
[ "$(col_order)" = "w1 | w2" ] || check "column order should be w1 | w2 (is $(col_order))"
# Beside another page there is nothing to share: no grip at the screen's edge.
hc layers -j | grep -q uq-resize && check "a resize grip is up at the screen's edge, beside a window on another page"
finish

# ---- tabmove ----------------------------------------------------------------------------------
begin tabmove
echo "tabs: $(tabs)"
read -r tx ty tw < <(tab_of w1)
read -r ux uy _ < <(tab_of w2)
[ -n "${tx:-}" ] && [ -n "${ux:-}" ] || fail "no tabs for w1/w2 ($(tabs))"
send "move $tx $ty" "sleep 300" "button 272 down" "sleep 100"
for i in $(seq 1 10); do send "move $((tx + (ux + tw / 2 + 10 - tx) * i / 10)) $ty" "sleep 40"; done
send "sleep 300" "button 272 up"
drain
ms=$(wait_ms 4 '[ "$(col_order)" = "w2 | w1" ]')
latency "$ms" "release until the columns swap"
settle
[ "$(col_order)" = "w2 | w1" ] || check "dragging w1's tab past w2's should swap their columns (order is $(col_order))"
[ "$(tab_order)" = "w2 | w1" ] || check "the tabs should read w2 | w1 after the drag (they read $(tab_order))"
[ "$(active)" = w1 ] || check "the dragged tab's window should have focus (active '$(active)')"
on_screen "$(active)" >/dev/null || check "the focused window should be on screen after the tab move"
finish

# ---- tabdrop ----------------------------------------------------------------------------------
# A third window (its own Full page), then w2 is focused (Half, alone on its page) and w3's tab is
# dragged down onto the screen: w3 fills the empty half beside w2.
open_window w3
wait_ms 10 '[ -n "$(addr_of w3)" ]' >/dev/null || fail "w3 never opened"
settle 2
dispatch "[\"focus_window\",{\"address\":\"$(addr_of w2)\"}]"
settle 2
begin tabdrop
echo "before: $(col_order); tabs $(tab_order)"
on_screen w2 >/dev/null && [ "$(wwidth w2)" -lt 700 ] || check "w2 should show Half before the drop"
read -r tx ty _ < <(tab_of w3)
[ -n "${tx:-}" ] || fail "no tab for w3 ($(tabs))"
send "move $tx $ty" "sleep 300" "button 272 down" "sleep 100"
for i in $(seq 1 12); do send "move $((tx + (900 - tx) * i / 12)) $((ty + (400 - ty) * i / 12))" "sleep 40"; done
send "sleep 600"; drain
layers_held=$(hc layers -j | jq -r '[.. | objects | select(has("namespace")) | .namespace] | unique | join(" ")')
echo "layers while held over the screen: $layers_held"
echo "$layers_held" | grep -q uq-tab-drop || check "no drop preview while the tab was held over the screen"
shot tabdrop-held
send "button 272 up"; drain
ms=$(wait_ms 5 '[ "$(active)" = w3 ] && on_screen w3')
latency "$ms" "drop until the window is on the page, focused"
settle 2
echo "after: $(col_order)"
on_screen w3 >/dev/null || check "w3 should be on screen after its tab was dropped there"
on_screen w2 >/dev/null || check "w2 should stay on screen beside the dropped window"
[ "$(wx w3)" -gt "$(wx w2)" ] || check "w3 should fill the empty right half beside w2 (w2 x=$(wx w2), w3 x=$(wx w3))"
[ "$(active)" = w3 ] || check "the dropped window should have focus (active is '$(active)')"
[ "$(wwidth w3)" -lt 700 ] || check "the dropped window should be Half (is $(wwidth w3) wide)"
hc layers -j | grep -q uq-tab-drop && check "the drop preview stayed after the drop"
finish

# ---- grip -------------------------------------------------------------------------------------
# The gap between the pair (w2 | w3) dragged 150 px right: w2 widens, w3 narrows, far edges stay.
begin grip
hc layers -j | grep -q uq-resize || check "no resize grip between the pair"
x1=$(wx w2); w1w=$(wwidth w2); x2=$(wx w3); w2w=$(wwidth w3)
gx=$(( (x1 + w1w + x2) / 2 )); gy=360
echo "gap between x=$((x1 + w1w)) and x=$x2: grabbing at $gx,$gy"
send "move $((gx - 40)) $gy" "sleep 150" "move $gx $gy" "sleep 400" "button 272 down" "sleep 100"
for i in $(seq 1 15); do send "move $((gx + i * 10)) $gy" "sleep 30"; done
send "sleep 200" "button 272 up"
drain
ms=$(wait_ms 4 '[ "$(wwidth w2)" -ge $((w1w + 120)) ]')
latency "$ms" "release until the columns have their new widths"
settle
nx1=$(wx w2); nw1=$(wwidth w2); nx2=$(wx w3); nw2=$(wwidth w3)
echo "w2 $x1+$w1w -> $nx1+$nw1; w3 $x2+$w2w -> $nx2+$nw2"
d=$(( nw1 - w1w )); [ "$d" -ge 120 ] && [ "$d" -le 180 ] || check "w2 should be ~150 px wider after the grip drag (was $w1w, is $nw1)"
d=$(( nx1 - x1 )); [ "${d#-}" -le 3 ] || check "w2's left edge should stay put ($x1 -> $nx1)"
d=$(( (nx2 + nw2) - (x2 + w2w) )); [ "${d#-}" -le 3 ] || check "w3's right edge should stay put ($((x2 + w2w)) -> $((nx2 + nw2)))"
on_screen w2 >/dev/null && on_screen w3 >/dev/null || check "both should stay on screen after the grip drag"
# Back to Half each, for what follows.
dispatch "[\"column_width\",{\"address\":\"$(addr_of w2)\",\"width\":\"half\"}]"
dispatch "[\"column_width\",{\"address\":\"$(addr_of w3)\",\"width\":\"half\"}]"
settle 2
finish

# ---- superdrag --------------------------------------------------------------------------------
begin superdrag
# Super+drag the window on the left half of the pair to the right half: the two swap halves, and
# nothing ends up stacked in another window's column.
left=$(clients | jq -r '[.[] | select(.workspace.id == 1 and .floating == false and .at[0] >= 0 and .at[0] < 640 and .hidden == false)] | sort_by(.at[0]) | .[0].title')
right=$(clients | jq -r '[.[] | select(.workspace.id == 1 and .floating == false and .at[0] >= 640 and .at[0] < 1280 and .hidden == false)] | sort_by(.at[0]) | .[0].title')
cols_before=$(clients | jq '[.[] | select(.workspace.id == 1 and .floating == false) | .at[0]] | unique | length')
echo "left half: $left, right half: $right; $cols_before columns; order $(col_order)"
read -r cx cy < <(clients | jq -r --arg t "$left" '.[] | select(.title==$t) | "\((.at[0] + .size[0]/2)|floor) \((.at[1] + .size[1]/2)|floor)"')
send "move $cx $cy" "sleep 300" "mod super down" "sleep 100" "button 272 down" "sleep 100"
for i in $(seq 1 12); do send "move $((cx + i * 55)) $cy" "sleep 40"; done
send "sleep 800"; drain
held_layers=$(hc layers -j | jq -r '[.. | objects | select(has("namespace")) | .namespace] | unique | join(" ")')
echo "layers while held: $held_layers"
snap "  held"
echo "$held_layers" | grep -q uq-tab-drop || check "no drop preview while the window was held"
shot superdrag-held
send "button 272 up" "sleep 80" "mod super up"; drain
ms=$(wait_ms 5 '[ "$(wx "$left")" -ge 640 ] && [ "$(win "$left" floating)" = false ] && on_screen "$left"')
latency "$ms" "release until the window is on the right half"
settle 2
cols_after=$(clients | jq '[.[] | select(.workspace.id == 1 and .floating == false) | .at[0]] | unique | length')
[ "$cols_after" = "$cols_before" ] || check "the dropped window was stacked into another column ($cols_before columns before, $cols_after after)"
[ "$(win "$left" floating)" = false ] || check "$left is still floating after the drop"
[ "$(wx "$left")" -ge 640 ] || check "$left should be on the right half after the drag (x=$(wx "$left"))"
on_screen "$left" >/dev/null || check "$left should be fully on screen after the drag"
[ "$(win "$left" size | jq -r '.[1]')" -ge 600 ] || check "$left should be a full-height column, not stacked"
[ "$(active)" = "$left" ] || check "the dragged window should keep focus (active is '$(active)')"
finish

# ---- minimise ---------------------------------------------------------------------------------
begin minimise
victim=$(active)
t0=$(now_ms)
dispatch_now "[\"minimise\",{\"address\":\"$(addr_of "$victim")\",\"from_ws\":1}]"
ms=$(wait_ms 4 '[ "$(tab_flag "$victim" m)" = true ]')
latency "$ms" "minimise until its tab is greyed"
shell_part "^movewindowv2>>" "any(.[]; .t == \"$victim\" and .m)" minimise
# Its tab must stay in the strip all along (greyed), not drop out and come back.
gone=$(grep 'UQPROBE ' "$tmp/qs.log" | sed 's/.*UQPROBE //' | awk -v t="$phase_t0" '$1 >= t' | cut -d' ' -f2- |
    jq -c --arg v "$victim" 'select(all(.[]; .t != $v))' | wc -l)
[ "$gone" = 0 ] || check "$victim's tab left the strip for a moment while it was minimised ($gone probe readings without it)"
settle
[ "$(win "$victim" workspace | jq -r .name)" = special:minimized ] || check "$victim should be on special:minimized"
[ "$(active)" != "$victim" ] && [ -n "$(active)" ] || check "focus should move to another window after minimising (active '$(active)')"
for w in $(clients | jq -r '.[] | select(.workspace.id == 1 and .hidden == false) | .title'); do
    edge_ok "$w" "after the minimise"
done
shot minimise-tab
# Restore: click its tab.
read -r tx ty _ < <(tab_of "$victim")
send "move $tx $ty" "sleep 300" "button 272 down" "sleep 80" "button 272 up"; drain
t1=$(now_ms)
ms2=$(wait_ms 5 '[ "$(active)" = "$victim" ]')
echo "restore click until focused: $ms2 ms"
settle 2
[ "$(win "$victim" workspace | jq -r .id)" = 1 ] || check "$victim should be back on workspace 1"
[ "$(active)" = "$victim" ] || check "the restored window should have focus (active '$(active)')"
on_screen "$victim" >/dev/null || check "the restored window should be on screen"
[ "$(tab_flag "$victim" m)" = false ] || check "the restored window's tab is still greyed"
[ "$(clients | jq -r '[.[] | select(.workspace.id == 1 and .floating == false)] | max_by(.at[0]) | .title')" = "$victim" ] \
    || check "the restored window should be the last column (order $(col_order))"
finish
echo "PERF restore-click latency=${ms2}ms"
printf '| restore | %s | click on the greyed tab until the window is focused | | | | | | |\n' "$ms2" >>"$tmp/perf.md"

# ---- float ------------------------------------------------------------------------------------
begin float
fw=$(active)
dispatch "[\"toggle_float\",{\"address\":\"$(addr_of "$fw")\"}]"
settle 1.5
[ "$(win "$fw" floating)" = true ] || check "$fw should be floating"
read -r cx cy < <(clients | jq -r --arg t "$fw" '.[] | select(.title==$t) | "\((.at[0] + .size[0]/2)|floor) \((.at[1] + 20)|floor)"')
# Super+drag it so half of it hangs past the right edge.
send "move $cx $cy" "sleep 300" "mod super down" "sleep 100" "button 272 down" "sleep 100"
for i in $(seq 1 12); do send "move $((cx + (1250 - cx) * i / 12)) $cy" "sleep 40"; done
send "sleep 300" "button 272 up" "sleep 80" "mod super up"; drain
ms=$(wait_ms 5 '[ $(( $(wx "$fw") + $(wwidth "$fw") )) -gt 1280 ]')
latency "$ms" "release until the floating window is where the pointer left it"
settle 2
# Keep windows on screen leaves a window the user put partly off screen where they put it.
[ $(( $(wx "$fw") + $(wwidth "$fw") )) -gt 1280 ] || check "the floating window $fw should stay where it was dragged, partly off screen (x=$(wx "$fw"))"
[ "$(win "$fw" floating)" = true ] || check "$fw should still float"
dispatch "[\"toggle_float\",{\"address\":\"$(addr_of "$fw")\"}]"
settle 2
[ "$(win "$fw" floating)" = false ] || check "$fw should be tiled again"
finish

# ---- fullscreen -------------------------------------------------------------------------------
begin fullscreen
fw=$(active)
dispatch_now "[\"fullscreen\",{\"address\":\"$(addr_of "$fw")\",\"mode\":\"full\"}]"
ms=$(wait_ms 4 '[ "$(win "$fw" fullscreen)" = 2 ]')
latency "$ms" "fullscreen dispatch until Hyprland reports it"
settle
[ "$(win "$fw" size | jq -r '.[0]')" = 1280 ] || check "fullscreen $fw should be 1280 wide"
hc layers -j | grep -q uq-resize && check "the resize grips should be gone over a fullscreen window"
dispatch "[\"fullscreen\",{\"address\":\"$(addr_of "$fw")\",\"mode\":\"full\"}]"
settle 2
[ "$(win "$fw" fullscreen)" = 0 ] || check "$fw should have left fullscreen"
finish

# ---- workspace --------------------------------------------------------------------------------
begin workspace
mw=$(active)
t0=$(now_ms)
dispatch_now "[\"move\",{\"address\":\"$(addr_of "$mw")\",\"workspace\":2}]"
ms=$(wait_ms 4 '[ "$(win "$mw" workspace | jq -r .id)" = 2 ]')
latency "$ms" "move to workspace 2 until Hyprland has it there"
settle
[ "$(hc activeworkspace -j | jq -r .id)" = 1 ] || check "moving a window away should not switch workspace"
for w in $(clients | jq -r '.[] | select(.workspace.id == 1 and .hidden == false) | .title'); do
    edge_ok "$w" "after $mw left"
done
hc dispatch 'hl.dsp.focus({ workspace = 2 })' >/dev/null; settle
[ "$(hc activeworkspace -j | jq -r .id)" = 2 ] || check "couldn't switch to workspace 2"
on_screen "$mw" >/dev/null || check "$mw should fill workspace 2"
[ "$(wwidth "$mw")" -ge 1200 ] || check "$mw alone on workspace 2 should be Full ($(wwidth "$mw") wide)"
shot workspace-2
dispatch "[\"move\",{\"address\":\"$(addr_of "$mw")\",\"workspace\":1}]"; settle
hc dispatch 'hl.dsp.focus({ workspace = 1 })' >/dev/null; settle
[ "$(win "$mw" workspace | jq -r .id)" = 1 ] || check "$mw should be back on workspace 1"
# A special workspace (the scratchpad): the window leaves the tabs, and comes back.
sw=$(active)
dispatch "[\"move\",{\"address\":\"$(addr_of "$sw")\",\"workspace\":\"special:magic\"}]"
settle
[ "$(win "$sw" workspace | jq -r .name)" = special:magic ] || check "$sw should be on special:magic"
[ -z "$(tab_of "$sw")" ] || check "a window on the scratchpad should have no tab"
hc dispatch 'hl.dsp.workspace.toggle_special("magic")' >/dev/null; settle
[ "$(hc monitors -j | jq -r '.[0].specialWorkspace.name')" = special:magic ] || check "the scratchpad didn't open"
on_screen "$sw" >/dev/null || check "$sw should be on screen with the scratchpad open"
shot scratchpad
hc dispatch 'hl.dsp.workspace.toggle_special("magic")' >/dev/null; settle
dispatch "[\"move\",{\"address\":\"$(addr_of "$sw")\",\"workspace\":1}]"; settle 2
[ "$(win "$sw" workspace | jq -r .id)" = 1 ] || check "$sw should be back on workspace 1"
[ -n "$(tab_of "$sw")" ] || check "$sw back from the scratchpad should have its tab again"
for w in $(clients | jq -r '.[] | select(.workspace.id == 1 and .hidden == false) | .title'); do
    edge_ok "$w" "after the scratchpad"
done
finish

# ---- close ------------------------------------------------------------------------------------
begin close
cw=$(active)
t0=$(now_ms)
dispatch_now "[\"close\",{\"address\":\"$(addr_of "$cw")\"}]"
ms=$(wait_ms 4 '[ -z "$(tab_of "$cw")" ]')
latency "$ms" "close until its tab is gone"
shell_part "^closewindow>>" "all(.[]; .t != \"$cw\")" close
settle 2
[ -z "$(addr_of "$cw")" ] || check "$cw should be closed"
[ -n "$(active)" ] || check "focus should go to another window after a close"
for w in $(clients | jq -r '.[] | select(.workspace.id == 1 and .hidden == false) | .title'); do
    edge_ok "$w" "after a close"
done
finish

# ---- after ------------------------------------------------------------------------------------
errs=$(grep -E 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|Cannot read property|is not a function|uq: .*(error|failed)|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? ' "$tmp/qs.log" | grep -v 'frame rendered' | sort | uniq -c | sort -rn | head -n 15)
[ -n "$errs" ] && { echo "--- shell warnings and errors"; echo "$errs"; }
echo "$errs" | grep -qE 'TypeError|ReferenceError|is not a type|Cannot read property|is not a function' && check "QML errors in the shell log"
echo "--- the shell's uq: messages"; grep 'uq: ' "$tmp/qs.log" | tail -n 20
echo "slivers of a neighbouring page's window seen at the screen's edge: $slivers"
echo; echo "--- timings"; cat "$tmp/perf.md"
[ -n "$cap" ] && cp "$tmp/perf.md" "$cap/windows-perf.md"
[ -n "$cap" ] && ls -l "$cap" | grep windows-
[ "$bad" = 0 ] && echo "windows-live: all checks held"
exit "$bad"
