#!/usr/bin/env bash
# Windows stay solid when you switch workspace or screen, on a real (headless) Hyprland.
# The workspace animation used to fade windows out and in, so the wallpaper showed through for a
# moment. This runs Hyprland twice with uQ's animation lines (from the generated.lua golden file):
# once as they are now ("after"), once with the workspace style put back to "fade" ("before").
# Each run has a green window on workspace 1 and a blue one on workspace 2 over a red background,
# slows the switch to ~2.5 s, and grabs frames while it runs. A frame pixel that is none of those
# three colours (or black, the cursor and edge strips) is a blend, which means a window was see-through. "after" must have under 10% in its worst frame (a fade is ~100%).
#
#   scripts/workspace-solid-live.sh
#
# With UQ_CAPTURE_DIR set (CI does) it keeps a GIF of each run. Needs the same as
# scripts/drag-live.sh (a DRM card; Hyprland, hyprctl, foot, jq, grim, python3). Exit 0 when the
# checks held; 77 when this environment can't run it; 1 when a check failed.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)

skip() { echo "SKIP: $*"; exit 77; }
for bin in Hyprland hyprctl jq foot grim python3; do
    command -v "$bin" >/dev/null 2>&1 || skip "$bin is not installed"
done
ls /dev/dri/card* >/dev/null 2>&1 || skip "no DRM device"
golden=$root/crates/uq-core/tests/golden/generated/default.lua
[ -f "$golden" ] || skip "no golden generated.lua"

tmp=$(mktemp -d /tmp/uql.XXXXXX)
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; wait 2>/dev/null; rm -rf "$tmp"; }
trap cleanup EXIT
fail() { echo "FAIL: $*"; exit 1; }
cap=${UQ_CAPTURE_DIR:-}

# Fraction of sampled pixels that match none of the flat colours, worst frame of a directory.
analyse() {
    python3 -I - "$1" <<'PY'
import sys, glob
flat = [(0xcc, 0, 0), (0, 0xcc, 0), (0, 0, 0xcc), (0, 0, 0)]
worst = 0.0
for f in sorted(glob.glob(sys.argv[1] + "/*.ppm")):
    d = open(f, "rb").read()
    # P6 <w> <h> 255 \n then raw RGB
    parts = d.split(b"\n", 3)
    w, h = map(int, parts[1].split())
    raw = parts[3]
    n = bad = 0
    for i in range(0, w * h, 7):
        r, g, b = raw[3 * i], raw[3 * i + 1], raw[3 * i + 2]
        n += 1
        if all(abs(r - c[0]) + abs(g - c[1]) + abs(b - c[2]) > 40 for c in flat):
            bad += 1
    worst = max(worst, bad / n)
print("%.4f" % worst)
PY
}

run() {  # run <label> <workspace style>
    local label=$1 style=$2
    local d=$tmp/$label; mkdir -p "$d/rt" "$d/frames"; chmod 700 "$d/rt"
    local rt=$d/rt
    {
        grep -E '^hl\.(curve|animation)\(' "$golden" | sed -E "/leaf = \"workspaces\"/ { s/speed = [0-9.]+/speed = 25/; s/style = \"[a-z]+\"/style = \"$style\"/ }"
    } >"$d/anim.lua"
    grep -q "style = \"$style\"" "$d/anim.lua" || fail "$label: the workspaces animation line was not found in the golden file"
    cat >"$d/hypr.lua" <<LUA
hl.monitor({ output = "", mode = "640x360", position = "auto", scale = 1 })
hl.config({
    general = { gaps_in = 0, gaps_out = 0, border_size = 0 },
    decoration = { rounding = 0, shadow = { enabled = false }, blur = { enabled = false },
                   active_opacity = 1.0, inactive_opacity = 1.0 },
    animations = { enabled = true },
    misc = { disable_hyprland_logo = true, disable_splash_rendering = true,
             force_default_wallpaper = 0, background_color = 0xffcc0000 },
})
require([[$d/anim.lua]])
LUA
    env XDG_RUNTIME_DIR="$rt" LIBGL_ALWAYS_SOFTWARE=1 Hyprland -c "$d/hypr.lua" >"$d/hypr.log" 2>&1 &
    local hp=$!; pids+=($hp)
    local sig="" wl=""
    for _ in $(seq 1 50); do
        sig=$(find "$rt/hypr" -mindepth 1 -maxdepth 1 -printf '%f\n' 2>/dev/null | head -n1)
        [ -n "$sig" ] && [ -S "$rt/hypr/$sig/.socket.sock" ] && break
        sleep 0.2
    done
    [ -n "$sig" ] || skip "Hyprland never opened its socket"
    for _ in $(seq 1 25); do
        wl=$(find "$rt" -maxdepth 1 -name 'wayland-*' ! -name '*.lock' -printf '%f\n' | head -n1)
        [ -n "$wl" ] && break; sleep 0.2
    done
    [ -n "$wl" ] || skip "no Wayland socket"
    hc() { env XDG_RUNTIME_DIR="$rt" HYPRLAND_INSTANCE_SIGNATURE="$sig" hyprctl "$@"; }
    [ -n "$(hc monitors -j | jq -r '.[0].name // empty')" ] || hc output create headless >/dev/null 2>&1
    sleep 0.5
    hc configerrors | grep -v '^\s*$' | sed "s/^/$label: config error: /"
    for _ in $(seq 1 15); do   # Hyprland's first-run notice is a window: close it
        local a; a=$(hc clients -j | jq -r '.[] | select(.class | test("hyprland")) | .address' | head -n1)
        [ -n "$a" ] && { hc dispatch "hl.dsp.window.close({ window = \"address:$a\" })" >/dev/null; sleep 0.5; break; }
        sleep 0.2
    done
    local color ws
    for ws in 1 2; do
        [ "$ws" = 1 ] && color=00cc00 || color=0000cc
        hc dispatch "hl.dsp.focus({ workspace = $ws })" >/dev/null; sleep 1
        env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" LIBGL_ALWAYS_SOFTWARE=1 \
            foot -T "ws$ws" -o "colors-dark.background=$color" -o "colors-dark.alpha=1" -o "colors-dark.cursor=$color $color" sleep 600 >"$d/foot$ws.log" 2>&1 &
        pids+=($!)
        for _ in $(seq 1 40); do hc clients -j | jq -e ".[] | select(.title == \"ws$ws\")" >/dev/null 2>&1 && break; sleep 0.25; done
        hc clients -j | jq -e ".[] | select(.title == \"ws$ws\")" >/dev/null 2>&1 || fail "$label: the window on workspace $ws never opened"
    done
    hc dispatch "hl.dsp.focus({ workspace = 1 })" >/dev/null
    sleep 4   # let every animation finish: the next change is the one that is measured

    # Grab frames while switching 1 -> 2 -> 1.
    (n=0; while :; do env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" grim -t ppm "$d/frames/$(printf %04d $n).ppm" 2>/dev/null; n=$((n + 1)); done) &
    local gp=$!; pids+=($gp)
    sleep 0.5
    hc dispatch "hl.dsp.focus({ workspace = 2 })" >/dev/null; sleep 3.5
    hc dispatch "hl.dsp.focus({ workspace = 1 })" >/dev/null; sleep 3.5
    kill "$gp" 2>/dev/null; wait "$gp" 2>/dev/null
    local frames; frames=$(ls "$d/frames" | wc -l)
    [ "$frames" -ge 4 ] || fail "$label: only $frames frames were grabbed"
    worst[$label]=$(analyse "$d/frames")
    echo "$label (workspaces style '$style'): $frames frames, worst frame has ${worst[$label]} of its pixels see-through"
    if [ -n "$cap" ]; then
        mkdir -p "$cap"
        command -v ffmpeg >/dev/null 2>&1 && ffmpeg -loglevel error -y -framerate 6 -i "$d/frames/%04d.ppm" \
            -vf "scale=480:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse" "$cap/workspace-solid-$label.gif" 2>&1 | tail -n 3
    fi
    kill "$hp" 2>/dev/null; wait "$hp" 2>/dev/null
}

declare -A worst
run before fade
run after slide

bad=0
# "before" is evidence, not a check: how Hyprland blends under software rendering is its own business.
awk -v a="${worst[after]}" 'BEGIN { exit !(a < 0.10) }' || { echo "FAIL: windows were see-through while switching (${worst[after]} of the pixels in the worst frame)"; bad=1; }
[ "$bad" = 0 ] && echo "workspace-solid-live: windows stayed solid (before: ${worst[before]}, after: ${worst[after]})"
exit "$bad"
