#!/usr/bin/env bash
# Logging out on a real (headless) Hyprland with the real base.lua: its shutdown hook must stop the
# shell before Hyprland closes its Wayland socket, so the shell ends on SIGTERM (a clean stop for
# systemd) and never sees "The Wayland connection broke" (exit 255, a failed unit at every logout).
#
#   scripts/shutdown-live.sh
#
# systemctl is a fake on PATH: `stop uq-shell.service` sends the running qs SIGTERM and waits for
# it, as systemd does. Exit 0 when it ran and the checks held; 77 when this environment can't run
# it; 1 when a check failed.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)

skip() { echo "SKIP: $*"; exit 77; }
for bin in Hyprland hyprctl qs timeout; do
    command -v "$bin" >/dev/null 2>&1 || skip "$bin is not installed"
done
ls /dev/dri/card* >/dev/null 2>&1 || skip "no DRM device"

tmp=$(mktemp -d /tmp/uqx.XXXXXX)
rt="$tmp/rt"; mkdir -m 700 "$rt"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; wait 2>/dev/null; rm -rf "$tmp"; }
trap cleanup EXIT
fail() { echo "FAIL: $*"; for f in hypr qs systemctl; do [ -f "$tmp/$f.log" ] && { echo "--- $f.log"; tail -n 30 "$tmp/$f.log"; }; done; exit 1; }

mkdir -p "$tmp/bin"
cat >"$tmp/bin/systemctl" <<SH
#!/bin/sh
echo "\$*" >> "$tmp/systemctl.log"
case "\$*" in
    *"stop uq-shell.service"*)
        p=\$(cat "$tmp/qs.pid" 2>/dev/null) || exit 0
        kill -TERM "\$p" 2>/dev/null
        i=0; while kill -0 "\$p" 2>/dev/null && [ \$i -lt 40 ]; do sleep 0.1; i=\$((i + 1)); done ;;
    *is-enabled*) exit 1 ;;
esac
exit 0
SH
printf '#!/bin/sh\nexit 0\n' >"$tmp/bin/dbus-update-activation-environment"
chmod +x "$tmp/bin/"*

mkdir -p "$tmp/shell"
cat >"$tmp/shell/shell.qml" <<'QML'
import QtQuick
import Quickshell
ShellRoot {
    PanelWindow { anchors.top: true; anchors.left: true; anchors.right: true; implicitHeight: 30; color: "#336" }
}
QML

cat >"$tmp/hypr.lua" <<LUA
require([[$root/config/hypr/base.lua]])
hl.monitor({ output = "", mode = "1280x720", position = "auto", scale = 1 })
hl.config({ misc = { disable_hyprland_logo = true, disable_splash_rendering = true } })
LUA
env -u UQ_DEV_SESSION XDG_RUNTIME_DIR="$rt" LIBGL_ALWAYS_SOFTWARE=1 PATH="$tmp/bin:$PATH" \
    Hyprland -c "$tmp/hypr.lua" >"$tmp/hypr.log" 2>&1 &
hypr=$!
pids+=("$hypr")
sig=""
for _ in $(seq 1 50); do
    kill -0 "$hypr" 2>/dev/null || { cat "$tmp/hypr.log"; skip "headless Hyprland exited at start"; }
    sig=$(find "$rt/hypr" -mindepth 1 -maxdepth 1 -printf '%f\n' 2>/dev/null | head -n1)
    [ -n "$sig" ] && [ -S "$rt/hypr/$sig/.socket.sock" ] && break
    sleep 0.2
done
[ -n "$sig" ] || fail "Hyprland never opened its socket"
wl=$(find "$rt" -maxdepth 1 -name 'wayland-*' ! -name '*.lock' -printf '%f\n' | head -n1)
[ -n "$wl" ] || fail "no Wayland socket"
hc() { env XDG_RUNTIME_DIR="$rt" HYPRLAND_INSTANCE_SIGNATURE="$sig" hyprctl "$@"; }

env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" HYPRLAND_INSTANCE_SIGNATURE="$sig" QT_QPA_PLATFORM=wayland \
    qs -p "$tmp/shell/shell.qml" >"$tmp/qs.log" 2>&1 &
qs=$!
pids+=("$qs")
echo "$qs" >"$tmp/qs.pid"
for _ in $(seq 1 50); do
    hc layers -j 2>/dev/null | grep -q '"namespace"' && break
    kill -0 "$qs" 2>/dev/null || fail "qs exited at start"
    sleep 0.2
done

hc dispatch 'hl.dsp.exit()' >/dev/null
for _ in $(seq 1 50); do kill -0 "$hypr" 2>/dev/null || break; sleep 0.2; done
kill -0 "$hypr" 2>/dev/null && fail "Hyprland didn't exit"
wait "$qs"; code=$?
pids=()

grep -q 'stop uq-shell.service uqlip.service' "$tmp/systemctl.log" || fail "the shutdown hook didn't stop the shell and uqlip"
grep -q 'Wayland connection broke' "$tmp/qs.log" && fail "the shell lost its compositor before it was stopped"
[ "$code" = 143 ] || fail "qs exit status $code, want 143 (SIGTERM from the stop)"
echo "PASS: logout stops the shell before Hyprland closes (qs ended on SIGTERM)"
