#!/usr/bin/env bash
# uq-screenshot on a real (headless) Hyprland with the real shell, driven by a virtual pointer
# (tests/live/tools/uqinput): every capture is saved, copied and shown in the floating preview
# (never a new window); Copy puts the image back on the clipboard, Discard deletes the file, the
# close button dismisses; a second capture works after the first (wl-copy is still serving the
# clipboard then).
#
#   scripts/screenshot-live.sh
#
# With UQ_CAPTURE_DIR set (CI does) it keeps a screenshot of each step and a GIF of the whole run.
# Needs the same as scripts/drag-live.sh. The bridge has no uqd here, so the terminal is the
# fallback `kitty -e`, which a stub turns into foot, and systemd-run (the launch scope) is a stub
# that just runs the command. Exit 0 when it ran and the checks held; 77 when this environment
# can't run it; 1 when a check failed.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)

skip() { echo "SKIP: $*"; exit 77; }
for bin in Hyprland hyprctl qs dbus-daemon jq foot socat gcc wayland-scanner grim wl-copy wl-paste cmp; do
    command -v "$bin" >/dev/null 2>&1 || skip "$bin is not installed"
done
ls /dev/dri/card* >/dev/null 2>&1 || skip "no DRM device"
uqctl=${UQCTL:-$root/target/debug/uqctl}
[ -x "$uqctl" ] || skip "no uqctl binary"

tmp=$(mktemp -d /tmp/uql.XXXXXX)
rt="$tmp/rt"; mkdir -m 700 "$rt"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; wait 2>/dev/null; rm -rf "$tmp"; }
trap cleanup EXIT
fail() { echo "FAIL: $*"; for f in hypr qs; do [ -f "$tmp/$f.log" ] && { echo "--- $f.log"; tail -n 40 "$tmp/$f.log"; }; done; exit 1; }

"$root/tests/live/tools/build-uqinput.sh" "$tmp/uqinput" || skip "uqinput didn't build"

# Stubs: the scope the launch runs in, and the terminal uQ falls back to.
mkdir -p "$tmp/bin" "$tmp/data/applications" "$tmp/state/uq"
cat >"$tmp/bin/systemd-run" <<'SH'
#!/bin/sh
while [ $# -gt 0 ] && [ "$1" != -- ]; do shift; done
shift
exec "$@"
SH
cat >"$tmp/bin/kitty" <<'SH'
#!/bin/sh
shift
exec foot "$@"
SH
# A stand-in for the uqview image viewer: the preview's Mark up and Open hand the capture to it.
cat >"$tmp/bin/uqview" <<SH
#!/bin/sh
printf '%s\n' "\$@" >"$tmp/uqview.args"
SH
chmod +x "$tmp/bin/systemd-run" "$tmp/bin/kitty" "$tmp/bin/uqview"
printf '[Desktop Entry]\nType=Application\nName=Kitty\nExec=kitty\nIcon=utilities-terminal\n' >"$tmp/data/applications/kitty.desktop"
# Recents and the pinned squares, as the shell saves them.
printf '{"v":1,"pins":["","","","",""],"recents":["foot"]}' >"$tmp/state/uq/lookseedo.json"

dbus-daemon --session --address="unix:path=$tmp/bus" --nofork --nopidfile >"$tmp/dbus.log" 2>&1 &
pids+=($!)

cat >"$tmp/hypr.lua" <<LUA
require([[$root/config/hypr/base.lua]])
hl.monitor({ output = "", mode = "1280x720", position = "auto", scale = 1 })
hl.config({ misc = { disable_hyprland_logo = true, disable_splash_rendering = true } })
LUA
env XDG_RUNTIME_DIR="$rt" LIBGL_ALWAYS_SOFTWARE=1 UQ_DEV_SESSION=1 PATH="$tmp/bin:$PATH" \
    Hyprland -c "$tmp/hypr.lua" >"$tmp/hypr.log" 2>&1 &
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

shell_env=(env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" HYPRLAND_INSTANCE_SIGNATURE="$sig"
    DBUS_SESSION_BUS_ADDRESS="unix:path=$tmp/bus" PATH="$tmp/bin:$(dirname "$uqctl"):$PATH"
    XDG_STATE_HOME="$tmp/state" XDG_DATA_HOME="$tmp/data" QT_QPA_PLATFORM=wayland LC_ALL=C.UTF-8)
"${shell_env[@]}" qs -p "$root/shell" --no-duplicate >"$tmp/qs.log" 2>&1 &
pids+=($!)
for _ in $(seq 1 50); do hc layers -j | grep -q uq-bar && break; sleep 0.2; done
hc layers -j | grep -q uq-bar || fail "the bar never appeared"
# Hyprland's own first-run notice is a floating window: close it.
for _ in $(seq 1 15); do
    a=$(hc clients -j | jq -r '.[] | select(.class | test("hyprland")) | .address' | head -n1)
    [ -n "$a" ] && { hc dispatch "hl.dsp.window.close({ window = \"address:$a\" })" >/dev/null; sleep 0.5; break; }
    sleep 0.2
done


cap=${UQ_CAPTURE_DIR:-}; grabber=""
shot() { :; }
if [ -n "$cap" ]; then
    mkdir -p "$cap/frames"
    shot() { env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" grim "$@" 2>/dev/null; }
    (n=0; while :; do shot "$cap/frames/$(printf %04d $n).png"; n=$((n + 1)); sleep 0.25; done) &
    grabber=$!; pids+=($!)
fi
still() { [ -n "$cap" ] && shot "$cap/screenshot-$1.png"; }
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
ipc() { "${shell_env[@]}" timeout 10 qs ipc -p "$root/shell" call uq "$1" lookseedo 2>/dev/null; }
layers() { hc layers -j | jq '[.. | objects | select(has("levels")) | .levels["3"][]? | select(.namespace == "uq-lookseedo")] | length'; }
wait_layers() { for _ in $(seq 1 40); do [ "$(layers)" -eq "$1" ] && return 0; sleep 0.2; done; return 1; }
active() { hc activewindow -j | jq -r '.class // ""'; }
bad=0
check() { echo "FAIL: $*"; bad=1; }

pics="$tmp/pics"
# The real script, the real shell; only the area picker is a stub (a person drags it).
printf '#!/bin/sh\necho "200,150 400x300"\n' >"$tmp/bin/slurp"; chmod +x "$tmp/bin/slurp"
snap() { # mode
    "${shell_env[@]}" UQ_SHOT_DIR="$pics" UQ_SHOT_SHELL="$root/shell" UQ_SHOT_SLURP="$tmp/bin/slurp" \
        timeout 30 "$root/packaging/uq-screenshot" "$1" >"$tmp/snap.out" 2>&1
}
newest() { ls -1t "$pics"/Screenshot_*.png 2>/dev/null | head -n1; }
nshots() { ls -1 "$pics"/Screenshot_*.png 2>/dev/null | wc -l; }
layers() { hc layers -j | grep -o '"namespace": *"uq-screenshot"' | wc -l; }
wait_layers() { for _ in $(seq 1 40); do [ "$(layers)" -eq "$1" ] && return 0; sleep 0.2; done; return 1; }
geom() { hc layers -j | jq -r '.. | objects | select(.namespace? == "uq-screenshot") | "\(.x) \(.y) \(.w) \(.h)"' | head -n1; }
clip_matches() { "${shell_env[@]}" timeout 5 wl-paste --type image/png 2>/dev/null | cmp -s - "$1"; }
click() { send "move $1 $2" "sleep 300" "button 272 down" "sleep 80" "button 272 up" "sleep 700"; drain; }
png_size() { od -An -tu1 -j16 -N8 "$1" | awk '{print ($1*16777216+$2*65536+$3*256+$4) "x" ($5*16777216+$6*65536+$7*256+$8)}'; }

# --- 1. the whole screen: saved, copied, previewed; no new window ------------------------------
clients=$(hc clients -j | jq length)
echo old | "${shell_env[@]}" wl-copy
snap screen || check "uq-screenshot screen failed: $(cat "$tmp/snap.out")"
f=$(newest)
[ -n "$f" ] && [ -s "$f" ] || check "nothing was saved in $pics"
wait_layers 1 || { check "the preview never appeared"
    echo "--- uq-screenshot output:"; cat "$tmp/snap.out"
    echo "--- ipc: $("${shell_env[@]}" timeout 10 qs ipc -p "$root/shell" call screenshot present "$f" copied 2>&1)"
    echo "--- qs.log (screenshot lines)"; grep -i "screenshot\|ShotPreview\|ERROR" "$tmp/qs.log" | tail -n 20
    hc layers -j | jq -c '.. | objects | .namespace? // empty' | sort -u | tr '\n' ' '; echo; }
sleep 1; still 1-preview
[ "$(hc clients -j | jq length)" = "$clients" ] || check "a window opened for the screenshot"
[ "$(png_size "$f")" = 1280x720 ] || check "the screen capture is $(png_size "$f"), not 1280x720"
clip_matches "$f" || check "the clipboard doesn't hold the screenshot"
read -r lx ly lw lh <<<"$(geom)"
echo "preview at $lx,$ly ${lw}x$lh"
[ "${lw:-0}" -ge 300 ] || check "the preview is not a card (${lw}x$lh)"

# --- 2. Copy puts it back on the clipboard ------------------------------------------------------
echo old | "${shell_env[@]}" wl-copy
clip_matches "$f" && check "the clipboard still held the screenshot after it was replaced"
btn_y=$((ly + lh - 12 - 14))
click $((lx + 12 + 40)) "$btn_y"
sleep 0.5
clip_matches "$f" || check "Copy did not put the screenshot back on the clipboard"
still 2-copied

# --- 3. a second capture while wl-copy still serves the clipboard -------------------------------
sleep 1.1
snap region || check "uq-screenshot region failed: $(cat "$tmp/snap.out")"
g=$(newest)
[ -n "$g" ] && [ "$g" != "$f" ] || check "the second capture saved nothing (the lock was still held?)"
[ "$(png_size "$g")" = 400x300 ] || check "the area capture is $(png_size "$g"), not 400x300"
[ "$(layers)" = 1 ] || check "the preview isn't showing the second capture"
clip_matches "$g" || check "the clipboard doesn't hold the second screenshot"
sleep 0.8; still 3-second

# --- 4. Discard (the last button) deletes the file ----------------------------------------------
read -r lx ly lw lh <<<"$(geom)"
btn_y=$((ly + lh - 12 - 14))
click $((lx + lw - 12 - 30)) "$btn_y"
sleep 0.3; still 4-discarded
[ ! -e "$g" ] || check "Discard left $g behind"
wait_layers 0 || check "the preview stayed after Discard"
[ -e "$f" ] || check "Discard deleted the wrong file"

# --- 5. the close button dismisses, and keeps the file ------------------------------------------
sleep 1.1
snap screen || check "a third capture failed: $(cat "$tmp/snap.out")"
wait_layers 1 || check "the preview never appeared for the third capture"
h=$(newest); n=$(nshots)
read -r lx ly lw lh <<<"$(geom)"
click $((lx + lw - 12 - 14)) $((ly + 12 + 14))
wait_layers 0 || check "the close button didn't dismiss the preview"
[ "$(nshots)" = "$n" ] || check "dismissing deleted a file"

# --- 5b. Mark up hands the capture to uqview and closes the preview --------------------------------
sleep 1.1
snap screen || check "a capture for Mark up failed: $(cat "$tmp/snap.out")"
wait_layers 1 || check "no preview for the Mark up capture"
m=$(newest)
read -r lx ly lw lh <<<"$(geom)"
click $((lx + 135)) $((ly + lh - 12 - 14))
wait_layers 0 || check "the preview stayed after Mark up"
for _ in $(seq 1 20); do [ -s "$tmp/uqview.args" ] && break; sleep 0.25; done
[ "$(cat "$tmp/uqview.args" 2>/dev/null)" = "$(printf -- '--markup\n%s' "$m")" ] ||
    check "Mark up ran uqview with: $(tr '\n' ' ' <"$tmp/uqview.args" 2>/dev/null)"
[ -e "$m" ] || check "Mark up deleted the capture"

# --- 6. it goes away by itself ------------------------------------------------------------------
# The pointer is still over where the card was: it stays while hovered, so move away first.
send "move 100 300" "sleep 300"; drain
sleep 1.1; snap screen || check "a fourth capture failed"
wait_layers 1 || check "no preview for the fourth capture"
for _ in $(seq 1 80); do [ "$(layers)" = 0 ] && break; sleep 0.25; done
[ "$(layers)" = 0 ] || check "the preview never went away by itself"

if [ -n "$grabber" ]; then
    sleep 0.5; kill "$grabber" 2>/dev/null
    command -v ffmpeg >/dev/null 2>&1 && ffmpeg -loglevel error -y -framerate 4 -i "$cap/frames/%04d.png" \
        -vf "scale=960:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse" "$cap/screenshot.gif" 2>&1 | tail -n 3
    rm -rf "$cap/frames"; ls -l "$cap"
fi
echo "--- qs.log (tail)"; tail -n 12 "$tmp/qs.log"
[ "$bad" = 0 ] && echo "screenshot-live: all checks held"
exit "$bad"
