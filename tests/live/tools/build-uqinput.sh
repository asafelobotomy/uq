#!/usr/bin/env bash
# Build tests/live/tools/uqinput into $1 (default ./uqinput). Needs gcc, wayland (wayland-scanner,
# libwayland-client) and libxkbcommon.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out=${1:-uqinput}
gen=$(mktemp -d)
trap 'rm -rf "$gen"' EXIT
for p in virtual-keyboard-unstable-v1 wlr-virtual-pointer-unstable-v1; do
    wayland-scanner client-header "$here/protocols/$p.xml" "$gen/$p-client-protocol.h"
    wayland-scanner private-code "$here/protocols/$p.xml" "$gen/$p-protocol.c"
done
# shellcheck disable=SC2046
gcc -O1 -Wall -Wextra -I"$gen" -o "$out" "$here/uqinput.c" "$gen"/*-protocol.c \
    $(pkg-config --cflags --libs wayland-client xkbcommon)
