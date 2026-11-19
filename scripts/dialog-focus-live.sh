#!/usr/bin/env bash
# Keyboard focus for a window that needs typing (a file manager's "New folder" dialog) on a real
# (headless) Hyprland with the real shell, driven by a virtual keyboard and pointer
# (tests/live/tools/uqinput). A modal dialog opens over its parent; text typed afterwards must reach
# the dialog, whatever the shell's popups did before and wherever the pointer or a click goes.
#
#   scripts/dialog-focus-live.sh
#
# With UQ_CAPTURE_DIR set (CI does) it keeps a GIF of the whole run.
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
qmlbin=$(command -v qml6 || command -v qml) || skip "qml6 is not installed"
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
still() { [ -n "$cap" ] && shot "$cap/dialog-focus-$1.png"; }

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
ipc() { "${shell_env[@]}" timeout 10 qs ipc -p "$root/shell" call uq "$1" "$2" 2>/dev/null; }
wait_active() { hc activewindow -j | jq -r '.class // ""'; }
bad=0
check() { echo "FAIL: $*"; bad=1; }


# The test window: a parent with a modal dialog that opens 0.8 s after it starts. It prints
# "parent:<text>" / "dialog:<text>" as each field changes.
app_log="$tmp/app.log"
start_app() {
    : >"$app_log"
    env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" QT_QPA_PLATFORM=wayland QT_FORCE_STDERR_LOGGING=1 LC_ALL=C.UTF-8 \
        "$qmlbin" "$root/tests/live/fixtures/dialog-focus.qml" >"$app_log" 2>&1 &
    app=$!; pids+=($app)
    for _ in $(seq 1 60); do
        hc clients -j | jq -e '.[] | select(.title == "focus-dialog")' >/dev/null 2>&1 && break
        sleep 0.25
    done
    hc clients -j | jq -e '.[] | select(.title == "focus-dialog")' >/dev/null 2>&1 || fail "the dialog never opened ($(tail -n 5 "$app_log"))"
    sleep 0.8
}
stop_app() { kill "$app" 2>/dev/null; wait "$app" 2>/dev/null; sleep 0.5; }
active_title() { hc activewindow -j | jq -r '.title // ""'; }
rect() { hc clients -j | jq -r --arg t "$1" '.[] | select(.title == $t) | "\(.at[0]) \(.at[1]) \(.size[0]) \(.size[1])"'; }
# Type text and say where it went; fails when the dialog did not get it.
expect_dialog() {
    local what=$1 word=$2
    send "type $word" "sleep 500"; drain
    if grep -q "dialog:.*$word" "$app_log"; then echo "ok: $what (dialog got '$word')"
    else check "$what: the dialog did not get '$word' (active: '$(active_title)'; app log: $(tail -n 6 "$app_log" | tr '\n' '|'))"; fi
    still "$3"
}
ipcs() { for s in "$@"; do ipc open "$s" >/dev/null; sleep 0.8; ipc close "$s" >/dev/null; sleep 0.6; done; }

# --- 1. the plain case -----------------------------------------------------------------------
start_app
[ "$(active_title)" = focus-dialog ] || check "plain: the dialog is not focused when it opens (active: '$(active_title)')"
expect_dialog "plain" abc 1-plain
stop_app

# --- 2. after the shell's popups have been open and closed ---------------------------------------
ipcs lookseedo appmenu quicksettings powermenu
start_app
[ "$(active_title)" = focus-dialog ] || check "after popups: the dialog is not focused when it opens (active: '$(active_title)')"
expect_dialog "after popups" def 2-after-popups
stop_app

# --- 3. a popup open when the dialog opens: the dialog must get the keyboard, and the popup must go -----
n=0
for popup in appmenu quicksettings lookseedo powermenu; do
    n=$((n + 1))
    ipc open "$popup" >/dev/null; sleep 0.8
    start_app
    [ "$(active_title)" = focus-dialog ] || check "$popup open: the dialog is not focused (active: '$(active_title)')"
    expect_dialog "$popup open" "g$n" 3-$popup-open
    ipc close "$popup" >/dev/null; sleep 0.5
    stop_app
done

# --- 4. pointer wandering: over the bar, the parent, the dialog; click on the parent --------------
start_app
read -r dx dy dw dh < <(rect focus-dialog)
read -r px py pw ph < <(rect focus-parent)
echo "dialog at $dx,$dy ${dw}x${dh}; parent at $px,$py ${pw}x${ph}"
send "move 640 10" "sleep 400" "move $((px + 20)) $((py + pw / 8))" "sleep 400" "move $((dx + 20)) $((dy + 20))" "sleep 400"; drain
expect_dialog "pointer wandering" jkl 4-wander
send "move $((px + 30)) $((py + ph - 30))" "sleep 300" "button 272 down" "sleep 80" "button 272 up" "sleep 700"; drain
echo "after a click on the parent, active: '$(active_title)'"
expect_dialog "click on the parent" mno 5-click-parent
stop_app


if [ -n "$grabber" ]; then
    sleep 0.5; kill "$grabber" 2>/dev/null
    command -v ffmpeg >/dev/null 2>&1 && ffmpeg -loglevel error -y -framerate 4 -i "$cap/frames/%04d.png" \
        -vf "scale=960:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse" "$cap/dialog-focus.gif" 2>&1 | tail -n 3
    rm -rf "$cap/frames"; ls -l "$cap"
fi
echo "--- qs.log (tail)"; tail -n 12 "$tmp/qs.log"
[ "$bad" = 0 ] && echo "dialog-focus-live: all checks held"
exit "$bad"
