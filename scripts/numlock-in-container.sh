#!/usr/bin/env bash
# CI entry point for scripts/numlock-live.sh (the job "numlock"), as root in an archlinux container
# given the vkms card and /dev/uinput, sharing the runner's /dev/input, /run/udev and network:
#   docker run --rm --network host --device "$card" --device /dev/uinput --device-cgroup-rule 'c 13:* rmw' \
#       -v /dev/input:/dev/input -v /run/udev:/run/udev:ro \
#       -e SMOKE_CARD="$card" -v "$PWD:/src" archlinux:latest bash /src/scripts/numlock-in-container.sh
set -eu
card=${SMOKE_CARD:?SMOKE_CARD must name the DRM card}
pacman -Syu --noconfirm --needed hyprland quickshell qt6-declarative python python-evdev seatd ttf-dejavu >/dev/null
id tester >/dev/null 2>&1 || useradd -m tester
chmod 666 "$card"
SEATD_VTBOUND=0 seatd -u tester -l error &
sleep 1
rc=0
for mode in stock uq; do
    echo "=========== numlock-live $mode"
    timeout 180 bash /src/scripts/numlock-live.sh "$mode" || rc=$?
done
kill %1 2>/dev/null || true
exit "$rc"
