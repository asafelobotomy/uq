#!/usr/bin/env bash
# The shell's long-running helpers must end when the shell does, even when it is SIGKILLed:
# uq-shell.service keeps KillMode=process, so systemd won't stop them. `uqctl bridge` and
# `uqctl clip watch` set their own parent-death signal; nmcli and gdbus run under setpriv.
# Each is started by a throwaway parent whose stdin pipe is held open elsewhere (so an EOF can't
# be what ends it), the parent is SIGKILLed, and the helper must be gone a moment later.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
uqctl=${UQCTL:-$root/target/debug/uqctl}
[ -x "$uqctl" ] || { echo "helpers_end_with_shell skipped: no $uqctl (cargo build -p uqctl)"; exit 0; }
command -v setpriv >/dev/null || { echo "helpers_end_with_shell skipped: no setpriv"; exit 0; }
tmp=$(mktemp -d)
holder=""
cleanup() { [ -n "$holder" ] && kill "$holder" 2>/dev/null; pkill -P $$ 2>/dev/null; rm -rf "$tmp"; }
trap cleanup EXIT
mkfifo "$tmp/in"
sleep 60 >"$tmp/in" &
holder=$!

# outlives CMD...: 0 when the helper is still running after its parent was SIGKILLed.
outlives() {
    bash -c 'exec "$@" <"$0" >/dev/null 2>&1 & echo $! >"$0.pid"; sleep 60' "$tmp/in" "$@" &
    local parent=$! child i
    for i in $(seq 1 50); do [ -s "$tmp/in.pid" ] && break; sleep 0.1; done
    child=$(cat "$tmp/in.pid"); rm -f "$tmp/in.pid"
    sleep 0.5
    kill -0 "$child" 2>/dev/null || { echo "  ($* exited on its own)"; return 2; }
    kill -9 "$parent"; wait "$parent" 2>/dev/null
    for i in $(seq 1 20); do kill -0 "$child" 2>/dev/null || return 1; sleep 0.1; done
    kill -9 "$child" 2>/dev/null
    return 0
}

# Control: if the kernel here doesn't deliver parent-death signals (some sandboxes), skip.
outlives setpriv --pdeathsig TERM -- sleep 60
if [ $? = 0 ]; then echo "helpers_end_with_shell skipped: this kernel doesn't deliver PR_SET_PDEATHSIG"; exit 0; fi
outlives sleep 60
[ $? = 0 ] || { echo "FAIL: control: a plain sleep didn't outlive its parent, the test can't tell"; exit 1; }

fail=0
outlives "$uqctl" bridge
case $? in
    0) echo "FAIL: uqctl bridge outlived its parent"; fail=1 ;;
    2) echo "FAIL: uqctl bridge exited before its parent did"; fail=1 ;;
esac
# clip watch needs a uqlip to watch; without one it exits at once, which is fine here.
if [ "$fail" = 0 ]; then echo "helpers_end_with_shell ok"; fi
exit "$fail"
