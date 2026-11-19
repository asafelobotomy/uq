#!/usr/bin/env bash
# look=see>do_ on a real (headless) Hyprland with the real shell, driven by a virtual keyboard and
# pointer (tests/live/tools/uqinput): Recents and the five pinned squares, what hides when typing
# starts, filling a square, and the terminal that drops from the pill for a `>` command.
#
#   scripts/lookseedo-live.sh
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
chmod +x "$tmp/bin/systemd-run" "$tmp/bin/kitty"
printf '[Desktop Entry]\nType=Application\nName=Kitty\nExec=kitty\nIcon=utilities-terminal\n' >"$tmp/data/applications/kitty.desktop"
# Recents and the pinned squares, as the shell saves them.
printf '{"v":1,"pins":["","","","",""],"recents":["foot"]}' >"$tmp/state/uq/lookseedo.json"

# Files to search: Documents is a link to another drive, as on many setups.
mkdir -p "$tmp/home" "$tmp/drive/Documents/Taxes"
: >"$tmp/drive/Documents/Taxes/Tax_Return-2026.pdf"; : >"$tmp/drive/Documents/cv-final.odt"
ln -s "$tmp/drive/Documents" "$tmp/home/Documents"

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
    XDG_STATE_HOME="$tmp/state" XDG_DATA_HOME="$tmp/data" UQ_SEARCH_ROOTS="$tmp/home" QT_QPA_PLATFORM=wayland LC_ALL=C.UTF-8)
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
still() { [ -n "$cap" ] && shot "$cap/lookseedo-$1.png"; }

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

# --- 1. open: Recents and the pinned squares ------------------------------------------------
[ "$(ipc open)" = ok ] || fail "qs ipc open lookseedo did not answer ok"
wait_layers 1 || fail "no uq-lookseedo layer after opening"
sleep 1.5
hc layers -j | jq -c '.. | objects | select(has("levels")) | .levels["3"][]? | select(.namespace == "uq-lookseedo") | {x, y, w, h}'
still 1-home

# --- 2. an empty square opens the picker, which pins what is chosen ---------------------------
# Square 2: the squares sit under the pill (44), the Recent list and the Pinned heading.
read -r lx ly < <(hc layers -j | jq -r '.. | objects | select(has("levels")) | .levels["3"][]? | select(.namespace == "uq-lookseedo") | "\(.x) \(.y)"' | head -n1)
echo "layer at $lx,$ly"
# The squares sit under the pill (44), the Recent list (a heading and one row: foot is the only seeded
# app installed) and the Pinned heading; square 2 is the second of five.
sqy=$((ly + 44 + 5 + 12 + 57 + 5 + 22 + 5 + 38))
send "move $((lx + 12 + 135 + 38)) $sqy" "sleep 300" "button 272 down" "sleep 80" "button 272 up" "sleep 600"
drain; still 2-picker
send "type fo" "sleep 600"; drain; still 3-picker-typed
send "key Return" "sleep 800"; drain
saved=$(cat "$tmp/state/uq/lookseedo.json" 2>/dev/null)
echo "state after pinning: $saved"
echo "$saved" | jq -e '.pins[1] != ""' >/dev/null || check "no app was pinned to square 2 ($saved)"
still 4-pinned

# --- 3. typing a search hides Recents and the squares -----------------------------------------
send "type fo" "sleep 800"; drain; still 5-searching
send "key Escape" "sleep 600"; drain
wait_layers 0 || check "Escape did not close look=see>do_"

# --- 3b. documents: found by name through a linked Documents folder, several words in any order ---
ipc open >/dev/null; wait_layers 1 || fail "look=see>do_ did not reopen for the file search"
sleep 2
send "type return tax" "sleep 1000"; drain; still 5b-documents
send "key Escape" "sleep 600"; drain
wait_layers 0 || check "Escape did not close look=see>do_ after the file search"

# --- 4. a > command drops the terminal; Enter opens it; clicking it hands over ---------------
ipc open >/dev/null; wait_layers 1 || fail "look=see>do_ did not reopen"
sleep 1
send "type > echo hello from look=see" "sleep 900"; drain; still 6-terminal-drop
before=$(hc clients -j | jq 'length')
send "key Return" "sleep 2500"; drain
after=$(hc clients -j | jq 'length')
[ "$after" -gt "$before" ] || check "Enter opened no terminal window ($before -> $after clients)"
term=$(hc clients -j | jq -c '.[] | select(.class == "foot")' | head -n1)
echo "terminal: $term"
[ "$(echo "$term" | jq -r '.floating')" = true ] || check "the terminal is not floating"
[ "$(echo "$term" | jq -r '.size[0]')" -ge 600 ] || check "the terminal is not as wide as the pill ($(echo "$term" | jq -c .size))"
[ "$(echo "$term" | jq -r '.at[1]')" -gt $((ly + 44)) ] || check "the terminal is not under the pill ($(echo "$term" | jq -c .at), pill at $ly)"
[ "$(layers)" = 1 ] || check "look=see>do_ closed when the terminal opened"
[ "$(active)" != foot ] || check "the terminal took focus from look=see>do_"
still 7-terminal-open
read -r tx ty < <(echo "$term" | jq -r '"\(.at[0] + 100) \(.at[1] + 100)"')
send "move $tx $ty" "sleep 300" "button 272 down" "sleep 80" "button 272 up" "sleep 1500"; drain
wait_layers 0 || check "look=see>do_ stayed after the terminal was clicked"
[ "$(active)" = foot ] || check "the terminal is not focused after the click (active: $(active))"
still 8-terminal-focused

if [ -n "$grabber" ]; then
    sleep 0.5; kill "$grabber" 2>/dev/null
    command -v ffmpeg >/dev/null 2>&1 && ffmpeg -loglevel error -y -framerate 4 -i "$cap/frames/%04d.png" \
        -vf "scale=960:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse" "$cap/lookseedo.gif" 2>&1 | tail -n 3
    rm -rf "$cap/frames"; ls -l "$cap"
fi
echo "--- qs.log (tail)"; tail -n 12 "$tmp/qs.log"
[ "$bad" = 0 ] && echo "lookseedo-live: all checks held"
exit "$bad"
