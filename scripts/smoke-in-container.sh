#!/usr/bin/env bash
# CI entry point for scripts/smoke-shell.sh, run as root inside a throwaway archlinux container that
# was given one DRM card (vkms, loaded on the runner) with --device:
#   docker run --rm --device "$card" -e SMOKE_CARD="$card" -e SMOKE_UID="$(id -u)" \
#       -v "$PWD:/src" archlinux:latest bash /src/scripts/smoke-in-container.sh
# Hyprland cannot start without a DRM device, even with only headless outputs (see smoke-shell.sh),
# and its DRM backend needs a seat, so this runs seatd for an unprivileged user (Hyprland refuses
# root), builds uqctl as that user and runs the smoke test. Exit status is smoke-shell.sh's (77 =
# skipped), or 1 if the setup fails.
set -eu
card=${SMOKE_CARD:?SMOKE_CARD must name the DRM card passed to the container}
# SMOKE_NO_INSTALL=1: the image already has these (a local run from a prepared image).
[ -n "${SMOKE_NO_INSTALL:-}" ] ||
    pacman -Syu --noconfirm --needed rust dbus hyprland quickshell qt6-declarative jq foot ttf-dejavu grim seatd >/dev/null
# Same uid as the checkout's owner on the runner, so the build can write /src/target.
id tester >/dev/null 2>&1 || useradd -m -u "${SMOKE_UID:-1000}" tester
# The node keeps the runner's root:video 0660, and the container's video group is another gid.
chmod 666 "$card"
# No VTs in a container: a VT-bound seat could never be opened.
SEATD_VTBOUND=0 seatd -u tester -l error &
rc=0
su tester -c "set -e
    cd /src
    cargo build --locked -p uqctl
    export XDG_RUNTIME_DIR=\$(mktemp -d) LIBSEAT_BACKEND=seatd UQCTL=/src/target/debug/uqctl
    timeout 300 scripts/smoke-shell.sh" || rc=$?
kill %1 2>/dev/null || true
exit "$rc"
