#!/usr/bin/env bash
# Edit mode's Bar editor dialog on a real (headless) Hyprland with the real shell, driven by a
# virtual keyboard and pointer (tests/live/tools/uqinput): it opens with edit mode, the screen
# under the bar is dimmed, Esc closes it with edit mode still on, S opens it again.
#
#   scripts/baredit-live.sh
#
# With UQ_CAPTURE_DIR set (CI does) it keeps a screenshot of each step and a GIF of the whole run.
# Needs the same as scripts/drag-live.sh. The bridge has no uqd here, so the terminal is the
# fallback `kitty -e`, which a stub turns into foot, and systemd-run (the launch scope) is a stub
# that just runs the command. Exit 0 when it ran and the checks held; 77 when this environment
# can't run it; 1 when a check failed.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)

skip() { echo "SKIP: $*"; exit 77; }
for bin in Hyprland hyprctl qs dbus-daemon jq foot socat gcc wayland-scanner; do
    command -v "$bin" >/dev/null 2>&1 || skip "$bin is not installed"
done
ls /dev/dri/card* >/dev/null 2>&1 || skip "no DRM device"
uqctl=${UQCTL:-$root/target/debug/uqctl}
[ -x "$uqctl" ] || skip "no uqctl binary"

tmp=$(mktemp -d /tmp/uqe.XXXXXX)
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
chmod +x "$tmp/bin/systemd-run" "$tmp/bin/kitty"
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
if [ -n "$cap" ] && command -v grim >/dev/null 2>&1; then
    mkdir -p "$cap/frames"
    shot() { env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" grim "$@" 2>/dev/null; }
    (n=0; while :; do shot "$cap/frames/$(printf %04d $n).png"; n=$((n + 1)); sleep 0.25; done) &
    grabber=$!; pids+=($!)
fi
still() { [ -n "$cap" ] && shot "$cap/baredit-$1.png"; }

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
ipc() { "${shell_env[@]}" timeout 10 qs ipc -p "$root/shell" call uq "$@" 2>/dev/null; }
layers() { hc layers -j | grep -o "\"namespace\": *\"$1\"" | wc -l; }
wait_layers() { for _ in $(seq 1 40); do [ "$(layers "$1")" -eq "$2" ] && return 0; sleep 0.2; done; return 1; }
bad=0
check() { echo "FAIL: $*"; bad=1; }

still 0-bar
# --- 1. edit mode on: the dialog opens with it, the panel waits -------------------------------
[ "$(ipc editBar)" = on ] || fail "qs ipc editBar did not answer on"
wait_layers uq-bar-edit 1 || fail "no edit panel after editBar"
wait_layers uq-bar-editor 1 || check "the Bar editor did not open with edit mode"
sleep 1
hc layers -j | jq -c '.. | objects | select(.namespace? == "uq-bar-editor") | {x, y, w, h}'
still 1-dialog

# --- 2. the pointer: an island on the left, All islands, an island type -----------------------
# Rows are laid out by the dialog; the pictures show where, and the first pass only looks.
read -r lx ly lw lh < <(hc layers -j | jq -r '.. | objects | select(.namespace? == "uq-bar-editor") | "\(.x) \(.y) \(.w) \(.h)"' | head -n1)
cx=$((lx + lw / 2)); cy=$((ly + lh / 2))
send "move $cx $cy" "sleep 400"; drain; still 2-pointer

# --- 3. Esc closes the dialog; edit mode stays ------------------------------------------------
send "key Escape" "sleep 800"; drain
wait_layers uq-bar-editor 0 || check "Esc did not close the Bar editor"
[ "$(layers uq-bar-edit)" = 1 ] || check "Esc on the dialog ended edit mode"
still 3-closed

# --- 4. S on the edit panel brings it back ----------------------------------------------------
send "type s" "sleep 800"; drain
wait_layers uq-bar-editor 1 || check "S did not open the Bar editor again"
still 4-reopened

# --- 5. Esc twice more: dialog, then edit mode ------------------------------------------------
send "key Escape" "sleep 800" "key Escape" "sleep 800"; drain
wait_layers uq-bar-editor 0 || check "the Bar editor stayed"
wait_layers uq-bar-edit 0 || check "Esc did not finish edit mode"
[ "$(ipc editBar)" = on ] || check "editBar after Esc was not a fresh start"
wait_layers uq-bar-editor 1 || check "edit mode a second time did not open the Bar editor"
still 5-again

if [ -n "$grabber" ]; then
    sleep 0.5; kill "$grabber" 2>/dev/null
    command -v ffmpeg >/dev/null 2>&1 && ffmpeg -loglevel error -y -framerate 4 -i "$cap/frames/%04d.png" \
        -vf "scale=960:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse" "$cap/baredit.gif" 2>&1 | tail -n 3
    rm -rf "$cap/frames"; ls -l "$cap"
fi
echo "--- qs.log (tail)"; tail -n 12 "$tmp/qs.log"
[ "$bad" = 0 ] && echo "baredit-live: all checks held"
exit "$bad"
