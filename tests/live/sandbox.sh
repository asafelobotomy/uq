#!/usr/bin/env bash
# Run the Settings live checks (tests/live/run.sh) against this checkout's uqd in a throwaway
# sandbox: a private D-Bus bus, private config/state/data dirs, and a headless Hyprland when there
# is a DRM card for it (CI's vkms; see tests/live/ci-in-container.sh). Without Hyprland the checks
# that need it are N/A and the rest run against uqd alone.
#
#   tests/live/sandbox.sh [run.sh options]
#
# UQD, UQCTL: the binaries (default target/debug/uqd and uqctl; build them first). Never touches the
# real session: nothing here uses the real bus, the real runtime dir or the real home.
set -uo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
uqd=${UQD:-$root/target/debug/uqd}
uqctl=${UQCTL:-$root/target/debug/uqctl}
for b in "$uqd" "$uqctl"; do [ -x "$b" ] || { echo "sandbox: no $b (cargo build -p uqd -p uqctl)" >&2; exit 2; }; done
for t in dbus-daemon busctl jq; do command -v "$t" >/dev/null || { echo "sandbox: $t is not installed" >&2; exit 2; }; done

# Short paths: unix socket names must stay under 108 bytes.
sb=$(mktemp -d /tmp/uqlive.XXXXXX)
mkdir -p "$sb/home" "$sb/config/hypr/uq" "$sb/state" "$sb/data" "$sb/share" "$sb/rt" "$sb/log" "$sb/bin" \
    "$sb/sysdata/applications" "$sb/sysconfig/autostart"
chmod 700 "$sb/rt"
ln -s "$root/config/hypr" "$sb/share/hypr"
ln -s "$root/themes" "$sb/share/themes"
ln -s "$root/templates" "$sb/share/templates"
touch "$sb/config/hypr/uq/generated.lua" "$sb/config/hypr/uq/user.lua"
pids=()
cleanup() {
    [ -f "$sb/uqd.pid" ] && kill "$(cat "$sb/uqd.pid")" 2>/dev/null
    for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done
    wait 2>/dev/null
    [ -n "${UQ_LIVE_KEEP:-}" ] && { echo "sandbox kept: $sb" >&2; return; }
    rm -rf "${sb:?}"
}
trap cleanup EXIT

export HOME=$sb/home XDG_CONFIG_HOME=$sb/config XDG_STATE_HOME=$sb/state XDG_DATA_HOME=$sb/data \
    XDG_RUNTIME_DIR=$sb/rt UQ_DATA_DIR=$sb/share DBUS_SESSION_BUS_ADDRESS=unix:path=$sb/rt/bus \
    PATH="$sb/bin:$(dirname "$uqctl"):$PATH" \
    XDG_DATA_DIRS="$sb/sysdata:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}" \
    XDG_CONFIG_DIRS="$sb/sysconfig:${XDG_CONFIG_DIRS:-/etc/xdg}"
unset HYPRLAND_INSTANCE_SIGNATURE WAYLAND_DISPLAY DISPLAY UQ_DEV_SESSION
dbus-daemon --session --address="$DBUS_SESSION_BUS_ADDRESS" --nofork --nopidfile >"$sb/log/dbus.log" 2>&1 &
pids+=($!)
for _ in $(seq 1 50); do [ -S "$sb/rt/bus" ] && break; sleep 0.1; done

# A headless Hyprland on the uQ config (base.lua, then uqd's generated.lua and user.lua), when one
# can start here. UQ_DEV_SESSION keeps base.lua from asking systemd to (re)start uQ's units.
if command -v Hyprland >/dev/null && ls /dev/dri/card* >/dev/null 2>&1; then
    {
        printf 'require([[%s]])\n' "$root/config/hypr/base.lua"
        printf 'require("./uq/generated.lua")\nrequire("./uq/user.lua")\n'
        printf 'hl.monitor({ output = "", mode = "1280x720", position = "auto", scale = 1 })\n'
        printf 'hl.config({ misc = { disable_hyprland_logo = true, disable_splash_rendering = true } })\n'
    } >"$sb/config/hypr/hyprland.lua"
    UQ_DEV_SESSION=1 LIBGL_ALWAYS_SOFTWARE=1 Hyprland -c "$sb/config/hypr/hyprland.lua" >"$sb/log/hyprland.log" 2>&1 &
    pids+=($!)
    sig=""
    for _ in $(seq 1 100); do
        sig=$(find "$sb/rt/hypr" -mindepth 1 -maxdepth 1 -printf '%f\n' 2>/dev/null | head -n1)
        [ -n "$sig" ] && [ -S "$sb/rt/hypr/$sig/.socket.sock" ] && break
        sleep 0.2
    done
    if [ -n "$sig" ]; then
        export HYPRLAND_INSTANCE_SIGNATURE=$sig
        for _ in $(seq 1 50); do
            w=$(find "$sb/rt" -maxdepth 1 -name 'wayland-*' ! -name '*.lock' -printf '%f\n' | head -n1)
            [ -n "$w" ] && break; sleep 0.1
        done
        [ -n "${w:-}" ] && export WAYLAND_DISPLAY=$w
        echo "sandbox: headless Hyprland up ($sig${w:+, $w})" >&2
    else
        echo "sandbox: Hyprland did not start; running without it (see below)" >&2
        tail -n 20 "$sb/log/hyprland.log" >&2
    fi
else
    echo "sandbox: no Hyprland or no DRM card; the checks that need Hyprland are N/A" >&2
fi

# uqd's stop/start for the checks that restart it (systemd does this on a real session).
cat >"$sb/bin/uqdctl" <<SH
#!/bin/sh
case "\$1" in
    stop) [ -f "$sb/uqd.pid" ] && kill "\$(cat "$sb/uqd.pid")" 2>/dev/null; rm -f "$sb/uqd.pid" ;;
    start) setsid "$uqd" >>"$sb/log/uqd.log" 2>&1 & echo \$! >"$sb/uqd.pid" ;;
esac
SH
chmod 755 "$sb/bin/uqdctl"
"$sb/bin/uqdctl" start
for _ in $(seq 1 100); do busctl --user status org.uq.Desktop1 >/dev/null 2>&1 && break; sleep 0.1; done

# The system's app and autostart folders as uqd sees them, for the checks that add system entries.
export UQ_LIVE_SYSDATA=$sb/sysdata UQ_LIVE_SYSCONFIG=$sb/sysconfig
export UQ_LIVE_SANDBOX=1 UQ_LIVE_UQDCTL=$sb/bin/uqdctl UQCTL=$uqctl UQ_LIVE_OUT=${UQ_LIVE_OUT:-$sb/out}
rc=0
"$root/tests/live/run.sh" "$@" || rc=$?
if [ "$rc" != 0 ]; then
    for f in uqd hyprland; do echo "--- $f.log (last 40 lines)"; tail -n 40 "$sb/log/$f.log" 2>/dev/null; done
fi
exit "$rc"
