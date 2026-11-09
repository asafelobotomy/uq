#!/usr/bin/env bash
# The user units must not start outside the current Hyprland instance: no [Install] section
# (base.lua's start hook, or the shell for uq-lock, starts the unit) and an ExecCondition on the instance socket.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
fail=0
for u in uq-shell uqd uq-lock; do
    f="$root/packaging/$u.service"
    [ -f "$f" ] || { echo "FAIL: $u.service missing"; fail=1; continue; }
    if grep -q '^\[Install\]' "$f"; then echo "FAIL: $u.service has an [Install] section"; fail=1; fi
    if ! grep -q '^ExecCondition=.*HYPRLAND_INSTANCE_SIGNATURE.*hypr/.*\.socket\.sock' "$f"; then
        echo "FAIL: $u.service lacks the Hyprland socket ExecCondition"; fail=1
    fi
    if command -v systemd-analyze >/dev/null 2>&1; then
        # Verify needs the binaries to exist; only unit syntax problems matter here.
        out=$(systemd-analyze --user verify "$f" 2>&1 | grep -v -e 'Command .* is not executable' -e 'No such file' -e '^$') || true
        case "$out" in *"Unknown"*|*"Invalid"*|*"Failed to parse"*) echo "FAIL: verify $u.service: $out"; fail=1;; esac
    fi
done
# The lock is restarted after a crash or kill, but not after its clean exit (exit 0 = unlocked).
f="$root/packaging/uq-lock.service"
if [ -f "$f" ]; then
    for want in '^Restart=always$' '^RestartPreventExitStatus=0$' '^RestartSec=1$' '^StartLimitIntervalSec=0$' \
        '^KillMode=process$' '^PartOf=graphical-session.target$' '^ExecStart=/usr/bin/qs -p /usr/share/uq/lock$'; do
        grep -q "$want" "$f" || { echo "FAIL: uq-lock.service lacks $want"; fail=1; }
    done
fi
[ "$fail" = 0 ] && echo "units ok"
exit "$fail"
