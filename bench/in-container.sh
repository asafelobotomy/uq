#!/usr/bin/env bash
# CI entry point for bench/live.py, run as root in a throwaway archlinux container given one DRM
# card (vkms, loaded on the runner), like scripts/smoke-in-container.sh:
#   docker run --rm --device "$card" -e SMOKE_CARD="$card" -e SMOKE_UID="$(id -u)" \
#       -v "$PWD:/work" archlinux:latest bash /work/head/bench/in-container.sh /work/out /work/base /work/head
# Every SRC (a checkout) is built in release mode and measured with this checkout's live.py, in
# rounds that alternate between them (ROUNDS, default 2), so a pull request's base and head see the
# same machine and the same drift. Results: OUTDIR/<basename of SRC>-<round>.json.
set -eu
out=${1:?usage: in-container.sh OUTDIR SRC...}; shift
card=${SMOKE_CARD:?SMOKE_CARD must name the DRM card passed to the container}
[ -n "${SMOKE_NO_INSTALL:-}" ] ||
    pacman -Syu --noconfirm --needed rust dbus hyprland quickshell qt6-declarative jq ttf-dejavu seatd python strace procps-ng pkgconf wayland libxkbcommon >/dev/null
id tester >/dev/null 2>&1 || useradd -m -u "${SMOKE_UID:-1000}" tester
chmod 666 "$card"
SEATD_VTBOUND=0 seatd -u tester -l error &
script=$(cd "$(dirname "$0")" && pwd)/live.py
mkdir -p "$out" && chown tester "$out"
rc=0
for src in "$@"; do
    su tester -c "cd '$src' && cargo build --release --locked -p uqctl -p uqd -p uqlip" || { echo "build of $src failed"; exit 1; }
done
for round in $(seq 1 "${ROUNDS:-2}"); do
    for src in "$@"; do
        echo "=========== $(basename "$src") round $round"
        su tester -c "set -e
            export LIBSEAT_BACKEND=seatd XDG_RUNTIME_DIR=\$(mktemp -d)
            timeout 900 python3 -I '$script' --src '$src' --out '$out/$(basename "$src")-$round.json'" || rc=$?
    done
done
kill %1 2>/dev/null || true
exit "$rc"
