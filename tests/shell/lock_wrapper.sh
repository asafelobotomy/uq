#!/usr/bin/env bash
# packaging/uq-lock-run, with a stub qs (UQ_LOCK_QS, test-only): Quickshell's PAM relay lines are
# dropped from both stdout and stderr (the PAM subprocess writes to both, past the logger), other
# lines pass through, qs's exit status is kept (exec: qs is the unit's main PID, so
# RestartPreventExitStatus=0 still means "unlocked"), and the filter is gone once qs exits.
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
echo 'quickshell.service.pam.subprocess: Starting pam session for user "u" with config "uq-lock"'
printf 'quickshell.service.pam.subprocess: Got response for request.'
echo 'quickshell.service.pam.subprocess: Failed to authenticate.'
echo 'WARN: unrelated line' >&2
echo 'INFO: unrelated stdout line'
exit $1
SH
    chmod +x "$tmp/qs"
}

for code in 3 0; do
    stub "$code"
    # Command substitution waits for the grep child too (it holds the pipe until EOF).
    out=$(UQ_LOCK_QS="$tmp/qs" "$wrapper" 2>&1); rc=$?
    [ "$rc" = "$code" ] || { echo "FAIL: exit $rc, want $code"; fail=1; }
    case "$out" in *quickshell.service.pam*|*"Got response"*|*"Relaying"*) echo "FAIL: PAM relay line got through: $out"; fail=1;; esac
    case "$out" in *"WARN: unrelated line"*) ;; *) echo "FAIL: unrelated stderr line lost: $out"; fail=1;; esac
    case "$out" in *"INFO: unrelated stdout line"*) ;; *) echo "FAIL: unrelated stdout line lost: $out"; fail=1;; esac
    grep -qx 'ARGS -p /usr/share/uq/lock' <<<"$out" || { echo "FAIL: wrong qs args: $out"; fail=1; }
done
# exec: the process systemd started is qs itself (same PID), not a shell waiting on it. In its own
# session, so the filter can be found afterwards: it must end with qs, not linger.
stub 0
setsid env UQ_LOCK_QS="$tmp/qs" "$wrapper" >"$tmp/out" 2>/dev/null &
pid=$!
wait "$pid"
sleep 0.5
grep -qx "PID $pid" "$tmp/out" || { echo "FAIL: qs is not the wrapper's PID ($pid): $(cat "$tmp/out")"; fail=1; }
# Zombies don't count: the grep has exited, but in a container whose PID 1 doesn't reap (CI) it
# stays <defunct> after being reparented.
left=$(for p in $(pgrep -s "$pid" 2>/dev/null); do
    case "$(ps -o stat= -p "$p" 2>/dev/null)" in Z*|"") ;; *) echo "$p" ;; esac
done)
[ -z "$left" ] || { echo "FAIL: processes left after qs exited: $(ps -o pid,args -p "${left//$'\n'/,}")"; fail=1; }

[ "$fail" = 0 ] && echo "lock_wrapper ok"
exit "$fail"
