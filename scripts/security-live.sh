#!/usr/bin/env bash
# The Security island while a scan runs, on a real (headless) Hyprland with the real Bar, driven by
# a virtual keyboard and pointer (tests/live/tools/uqinput).
#
#   scripts/security-live.sh
#
# A stub `oyst-cli` answers `status summary` the way oysterAV does, with a scan that advances 4%
# a second once `oyst-cli scan` has been run from the island's popup. Checks: the island reads
# "Scanning NN%" while it runs and "Protected" after, and the popup opens with the bar showing.
# With UQ_CAPTURE_DIR set (CI does) it keeps screenshots and a GIF. Exit 0 when it ran and the
# checks held; 77 when this environment can't run it; 1 when a check failed.
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

mkdir -p "$tmp/bin" "$tmp/oyst" "$tmp/data/icons/hicolor/48x48/apps"
# oysterAV's own icon, so the island shows it the way it does on a desktop with oysterAV installed.
cp "$root/tests/live/fixtures/oysterav.png" "$tmp/data/icons/hicolor/48x48/apps/oysterav.png"
echo -1 >"$tmp/oyst/pct"
cat >"$tmp/bin/oyst-cli" <<SH
#!/bin/sh
d="$tmp/oyst"
case "\$1 \$2" in
"status summary")
    p=\$(cat "\$d/pct")
    if [ "\$p" -ge 0 ]; then
        s='"scanning":true,"scan_progress":{"percent":'\$p',"pack":"clamav","message":"Scanning /home","state":"running"}'
    else
        s='"scanning":false,"scan_progress":null'
    fi
    echo '{"schema":1,"state":"ok","reasons":[],'\$s',"last_scan_age_hours":2,"last_scan_profile":"quick","last_scan_state":"done","open_findings":0,"signature_age_hours":3,"signatures_stale":false,"quarantine_count":0,"checks":[],"setup_completed":true}'
    ;;
"scan --profile")
    ( i=0; while [ \$i -le 100 ]; do echo \$i >"\$d/pct"; i=\$((i + 4)); sleep 1; done; echo -1 >"\$d/pct" ) >/dev/null 2>&1 &
    ;;
esac
SH
chmod +x "$tmp/bin/oyst-cli"

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
                start: [{ id: "security", size: "medium" }], center: [], end: [], options: {}
            } } };
        }
    }
    Timer { interval: 300; running: true; repeat: true; triggeredOnStart: true; onTriggered: feed.fill() }
    Timer {
        interval: 500; running: true; repeat: true
        onTriggered: {
            var b = bars.instances[0];
            var s = b ? b.slotFor("start:0") : null;
            if (s && s.island) {
                var p = s.island.mapToItem(null, 0, 0);
                console.log("SLOT", Math.round(p.x), Math.round(p.y), Math.round(s.width), Math.round(s.height));
                console.log("WORD", s.island.shown, s.island.word);
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
    XDG_STATE_HOME="$tmp/state" XDG_DATA_DIRS="$tmp/data:/usr/local/share:/usr/share" QT_QPA_PLATFORM=wayland LC_ALL=C.UTF-8)
"${shell_env[@]}" qs -p "$tmp/harness/shell.qml" --no-duplicate >"$tmp/qs.log" 2>&1 &
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
still() { [ -n "$cap" ] && shot "$cap/security-$1.png"; }

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
bad=0
check() { echo "FAIL: $*"; bad=1; }
word() { grep 'WORD' "$tmp/qs.log" | tail -n1 | sed 's/.*WORD [a-z]* //'; }
wait_word() {   # wait_word REGEX SECONDS
    for _ in $(seq 1 $(($2 * 4))); do word | grep -Eq "$1" && return 0; sleep 0.25; done
    return 1
}

for _ in $(seq 1 40); do grep -q 'WORD true' "$tmp/qs.log" && break; sleep 0.25; done
grep -q 'WORD true' "$tmp/qs.log" || fail "the Security island never appeared"
read -r bx by < <(hc layers -j | jq -r '.. | objects | select(.namespace? == "uq-bar") | "\(.x) \(.y)"' | head -n1)
read -r sx sy sw sh < <(grep 'SLOT' "$tmp/qs.log" | tail -n1 | sed 's/.*SLOT //')
cx=$((bx + sx + sw / 2)); cy=$((by + sy + sh / 2))
echo "island at $cx,$cy; word: $(word)"
word | grep -q 'Protected' || check "idle island should read Protected, reads: $(word)"
sleep 1; still 1-idle

click() { send "move $1 $2" "sleep 300" "button 272 down" "sleep 80" "button 272 up" "sleep 900"; drain; }
# The popup's first row (Run a quick scan) has focus: Enter starts the scan and closes the popup.
click $cx $cy
send "key Return" "sleep 500"; drain
wait_word 'Scanning' 12 || check "the island never said Scanning, reads: $(word)"
sleep 6; still 2-scanning
wait_word 'Scanning [0-9]+%' 12 || check "the island never showed a percentage, reads: $(word)"
click $cx $cy
still 3-popup-progress
send "key Escape" "sleep 500"; drain
# The scan runs 25 s; the island goes back to Protected.
wait_word 'Protected' 40 || check "the island never went back to Protected, reads: $(word)"
still 4-done

grep -qE 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|Cannot read property|is not a function|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? ' "$tmp/qs.log" &&
    { grep -E 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|Cannot read property|is not a function|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? ' "$tmp/qs.log" | head -n 10; check "QML errors"; }
if [ -n "$grabber" ]; then
    sleep 0.5; kill "$grabber" 2>/dev/null
    command -v ffmpeg >/dev/null 2>&1 && ffmpeg -loglevel error -y -framerate 4 -i "$cap/frames/%04d.png" \
        -vf "scale=960:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse" "$cap/security-scan.gif" 2>&1 | tail -n 3
    rm -rf "$cap/frames"; ls -l "$cap"
fi
echo "--- qs.log (tail)"; tail -n 12 "$tmp/qs.log"
[ "$bad" = 0 ] && echo "security-live: all checks held"
exit "$bad"
