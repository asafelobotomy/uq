#!/usr/bin/env bash
# CI entry point for the Settings live checks (tests/live/sandbox.sh), run as root inside a
# throwaway archlinux container that was given one DRM card (vkms, loaded on the runner):
#   docker run --rm --device "$card" -e SMOKE_CARD="$card" -e SMOKE_UID="$(id -u)" \
#       -v "$PWD:/src" archlinux:latest bash /src/tests/live/ci-in-container.sh [run.sh options]
# Like scripts/smoke-in-container.sh: seatd for an unprivileged user (Hyprland refuses root), then
# this checkout's uqd and uqctl are built and the checks run against a headless Hyprland on the
# uQ config. Exit status is run.sh's.
set -eu
card=${SMOKE_CARD:?SMOKE_CARD must name the DRM card passed to the container}
[ -n "${SMOKE_NO_INSTALL:-}" ] ||
    pacman -Syu --noconfirm --needed rust dbus hyprland jq foot ttf-dejavu seatd gtk3 >/dev/null
id tester >/dev/null 2>&1 || useradd -m -u "${SMOKE_UID:-1000}" tester
chmod 666 "$card"
SEATD_VTBOUND=0 seatd -u tester -l error &
rc=0
su tester -c "set -e
    cd /src
    cargo build --locked -p uqd -p uqctl
    export LIBSEAT_BACKEND=seatd UQ_LIVE_OUT=/src/target/uq-live
    timeout 900 tests/live/sandbox.sh --slow $*" || rc=$?
kill %1 2>/dev/null || true
exit "$rc"
