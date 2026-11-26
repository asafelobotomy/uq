#!/usr/bin/env bash
# Num Lock across keyboards on a real Hyprland, with real (uinput) kernel keyboards. Run as root in
# a privileged container that shares the runner's /dev/input, /run/udev and network (udev events),
# with a vkms card and seatd for the tester user (see the CI job "numlock").
#   numlock-live.sh stock  -> plain numlock_by_default; exits 0 when the bug shows (expected)
#   numlock-live.sh uq     -> base.lua's numlock-devices block + emobie's lock sync; must pass
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
mode=${1:?stock or uq}
work=$(mktemp -d /tmp/numlock.XXXXXX)
chmod 755 "$work"
cat > "$work/hypr.lua" <<'LUA'
hl.monitor({ output = "", mode = "1280x720", position = "auto", scale = 1 })
hl.config({
    misc = { disable_hyprland_logo = true, disable_splash_rendering = true },
    animations = { enabled = false },
    input = { kb_layout = "us", numlock_by_default = true, follow_mouse = 0 },
})
LUA
[ "$mode" = uq ] && sed -n '/-- numlock-devices:begin/,/-- numlock-devices:end/p' "$root/config/hypr/base.lua" >> "$work/hypr.lua"
cp "$root/scripts/numlock/client.qml" "$work/client.qml"
python3 "$root/scripts/numlock/driver.py" "$mode" "$work/hypr.lua" "$work/client.qml" "$work"
rc=$?
pkill -u tester qs 2>/dev/null; pkill -u tester Hyprland 2>/dev/null
if [ "$mode" = stock ]; then
    [ "$rc" != 0 ] && { echo "stock Hyprland shows the bug, as expected"; exit 0; }
    echo "NOTE: stock Hyprland no longer shows the bug"; exit 0
fi
[ "$rc" = 0 ] || { echo "--- hypr.log"; tail -n 40 "$work/hypr.log"; echo "--- client.log"; tail -n 20 "$work/client.log"; }
exit "$rc"
