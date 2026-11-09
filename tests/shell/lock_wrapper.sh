#!/usr/bin/env bash
# packaging/uq-lock-run, with a stub qs (UQ_LOCK_QS, test-only): Quickshell's PAM relay lines are
# dropped from stderr, other stderr lines and stdout pass through, and qs's exit status is kept
# (exec: qs is the unit's main PID, so RestartPreventExitStatus=0 still means "unlocked").
# It never runs the real lock.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
wrapper="$root/packaging/uq-lock-run"
[ -x "$wrapper" ] || { echo "FAIL: packaging/uq-lock-run missing or not executable"; exit 1; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fail=0

stub() { # exit code
    cat > "$tmp/qs" <<SH
#!/bin/sh
echo "ARGS \$*"
echo "PID \$\$"
echo 'quickshell.service.pam.subprocess: Relaying pam message: "Password: " echo: 0 error: 0 responseRequired: 1' >&2
echo 'WARN: unrelated line' >&2
exit $1
SH
    chmod +x "$tmp/qs"
}

for code in 3 0; do
    stub "$code"
    # Command substitution waits for the grep child too (it holds the pipe until EOF).
    err=$(UQ_LOCK_QS="$tmp/qs" "$wrapper" 2>&1 >"$tmp/out"); rc=$?
    [ "$rc" = "$code" ] || { echo "FAIL: exit $rc, want $code"; fail=1; }
    case "$err" in *quickshell.service.pam*) echo "FAIL: PAM relay line reached stderr: $err"; fail=1;; esac
    case "$err" in *"WARN: unrelated line"*) ;; *) echo "FAIL: unrelated stderr line lost: $err"; fail=1;; esac
    grep -qx 'ARGS -p /usr/share/uq/lock' "$tmp/out" || { echo "FAIL: wrong qs args: $(cat "$tmp/out")"; fail=1; }
done
# exec: the process systemd started is qs itself (same PID), not a shell waiting on it.
stub 0
UQ_LOCK_QS="$tmp/qs" "$wrapper" >"$tmp/out" 2>/dev/null &
pid=$!
wait "$pid"
grep -qx "PID $pid" "$tmp/out" || { echo "FAIL: qs is not the wrapper's PID ($pid): $(cat "$tmp/out")"; fail=1; }

[ "$fail" = 0 ] && echo "lock_wrapper ok"
exit "$fail"
