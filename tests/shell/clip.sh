#!/usr/bin/env bash
# uq-clipd against a real compositor (headless sway): it records text and images, skips copies a
# password manager marked secret, skips ignored apps and pauses, puts an entry back with
# `uqctl clip copy`, and keeps pinned entries through `clear`. Skipped without sway and wl-clipboard.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
for c in sway wl-copy wl-paste grim jq; do
    command -v "$c" >/dev/null || { echo "clip skipped (no $c)"; exit 0; }
done
bin="$root/target/debug"
[ -x "$bin/uq-clipd" ] && [ -x "$bin/uqctl" ] || (cd "$root" && cargo build --locked -q -p uq-clip -p uqctl --examples) || exit 1
[ -x "$bin/examples/offer" ] || (cd "$root" && cargo build --locked -q -p uq-clip --examples) || exit 1

tmp=$(mktemp -d)
pids=()
trap 'kill "${pids[@]}" 2>/dev/null; rm -rf "$tmp"' EXIT
export XDG_RUNTIME_DIR="$tmp/run" XDG_DATA_HOME="$tmp/data" XDG_CONFIG_HOME="$tmp/cfg"
mkdir -m 700 "$XDG_RUNTIME_DIR"
export WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman
fail=0
bad() { echo "FAIL: $*"; fail=1; }

sway -c /dev/null >"$tmp/sway.log" 2>&1 & pids+=($!)
for _ in $(seq 50); do
    sock=$(ls "$XDG_RUNTIME_DIR"/wayland-* 2>/dev/null | grep -v '\.lock' | head -1)
    [ -n "$sock" ] && break
    sleep 0.2
done
[ -n "${sock:-}" ] || { echo "clip skipped (sway did not start)"; cat "$tmp/sway.log"; exit 0; }
export WAYLAND_DISPLAY=$(basename "$sock")

start_clipd() {
    [ -n "${clipd:-}" ] && { kill "$clipd"; wait "$clipd" 2>/dev/null; }
    "$bin/uq-clipd" >"$tmp/clipd.log" 2>&1 & clipd=$!
    pids+=("$clipd")
    for _ in $(seq 50); do "$bin/uqctl" clip status >/dev/null 2>&1 && return; sleep 0.1; done
    bad "uq-clipd did not start"; cat "$tmp/clipd.log"
}
clip() { "$bin/uqctl" clip "$@"; }
previews() { clip list --json | jq -r '.[].preview' | paste -sd'|'; }
settle() { sleep 0.6; }

start_clipd
echo hello | wl-copy; settle
[ "$(previews)" = "hello" ] || bad "plain text not recorded: $(previews)"

# A password manager's copy: the hint type is offered, so nothing is kept.
"$bin/examples/offer" 2 hunter2 text/plain x-kde-passwordManagerHint & pids+=($!)
sleep 1.2
[ "$(previews)" = "hello" ] || bad "a secret was recorded: $(previews)"
wait $! 2>/dev/null

# The same text without the hint is a normal copy.
"$bin/examples/offer" 2 visible text/plain & pids+=($!)
sleep 1.2
[ "$(previews)" = "visible|hello" ] || bad "normal offer not recorded: $(previews)"
wait $! 2>/dev/null

clip pause; echo paused | wl-copy; settle; clip resume
[ "$(previews)" = "visible|hello" ] || bad "recorded while paused: $(previews)"

grim "$tmp/shot.png" && wl-copy --type image/png < "$tmp/shot.png"; settle
[ "$(clip list --kind image --json | jq length)" = 1 ] || bad "image not recorded"
img=$(clip list --kind image --json | jq -r '.[0].id')
cmp -s "$(clip get "$img")" "$tmp/shot.png" || bad "stored image differs from the copy"

id=$(clip list hello --json | jq -r '.[0].id')
echo other | wl-copy; settle
clip copy "$id" && settle
[ "$(wl-paste)" = "hello" ] || bad "copy did not restore the entry: $(wl-paste)"

clip pin "$id"; clip clear >/dev/null
[ "$(previews)" = "hello" ] || bad "clear should keep only the pinned entry: $(previews)"
clip clear --all >/dev/null
[ "$(clip list --json | jq length)" = 0 ] || bad "clear --all left entries"

# An ignored app (the daemon asks hyprctl for the focused class; UQ_CLIP_TEST_APP stands in).
export UQ_CLIP_TEST_APP=keepassxc
start_clipd
echo fromkeepass | wl-copy; settle
[ "$(clip list --json | jq length)" = 0 ] || bad "a copy from an ignored app was recorded"

[ "$fail" = 0 ] && echo "clip ok"
exit "$fail"
