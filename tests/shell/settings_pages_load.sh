#!/usr/bin/env bash
# Every built-in Settings page loads: `uq-settings --screenshots` shows each page offscreen and
# exits 1 if one didn't load (a QML error, or a Kirigami type the installed Kirigami lacks, as
# Kirigami.Avatar on the Users page with Kirigami 6.31). Uses the cargo-built binary; skipped
# when there is none (cargo test --workspace builds it first in CI).
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
bin=""
for b in "${CARGO_TARGET_DIR:-$root/target}"/{debug,release}/uq-settings; do
    [ -x "$b" ] && { bin=$b; break; }
done
[ -n "$bin" ] || { echo "settings_pages_load skipped (no uq-settings build)"; exit 0; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/shots" "$tmp/state" "$tmp/config" "$tmp/rt"
chmod 700 "$tmp/rt"
# A private home for the window state; no session bus, so nothing reaches a running uqd.
env -u DBUS_SESSION_BUS_ADDRESS -u UQ_SETTINGS_SHOTS_BUS QT_QPA_PLATFORM=offscreen QT_FORCE_STDERR_LOGGING=1 \
    XDG_STATE_HOME="$tmp/state" XDG_CONFIG_HOME="$tmp/config" XDG_RUNTIME_DIR="$tmp/rt" \
    timeout 300 "$bin" --screenshots "$tmp/shots" >"$tmp/log" 2>&1
rc=$?
fail=0
if [ "$rc" != 0 ]; then
    echo "FAIL: uq-settings --screenshots exited $rc"
    grep -E "didn't load" "$tmp/log" | head -n 20
    fail=1
fi
n=$(find "$tmp/shots" -name '*.png' | wc -l)
[ "$n" -ge 20 ] || { echo "FAIL: only $n page screenshots"; fail=1; }
[ "$fail" = 0 ] && echo "settings_pages_load ok ($n pages)"
exit "$fail"
