#!/usr/bin/env bash
# packaging/uq-greeter-session with stub Hyprland, quickshell, hyprctl, localectl and agreety
# (UQ_GREETER_* overrides, test-only). The stub Hyprland runs the config's hyprland.start hook the
# way the real one would: `uq-greeter-session --shell` with UQ_GREETER_STATUS set.
#   - a launched session: exit 0, no failure recorded, Hyprland gets the wrapper config with the
#     localectl layout, and Hyprland is told to exit once the greeter has
#   - a greeter that exits without a session: exit 1 and a failure recorded
#   - 3 failures in 60 s: the third run execs `agreety --cmd /usr/bin/uq-session`; the next run
#     goes straight to agreety; failures older than 60 s don't count
#   - a greeter that never gets ready is stopped after the timeout and counts as a failure
#   - a hostile localectl layout never reaches the config
#   - Quickshell's greetd log lines are dropped
#   - /etc/uq/greeter-monitors.toml (UQ_GREETER_MONITORS): a good file's hl.monitor lines reach the
#     config (through the real `uqctl greeter monitors-lua` when target/debug/uqctl is built); a bad
#     or missing file, a failing uqctl or stray output leaves the automatic layout
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
script="$root/packaging/uq-greeter-session"
[ -x "$script" ] || { echo "FAIL: packaging/uq-greeter-session missing or not executable"; exit 1; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fail=0
mkdir -p "$tmp/bin" "$tmp/state" "$tmp/rt"

cat > "$tmp/bin/Hyprland" <<SH
#!/bin/bash
echo "Hyprland \$*" >> "$tmp/calls"
cp "\$2" "$tmp/last-config.lua"
# The hyprland.start hook.
"$script" --shell
SH
cat > "$tmp/bin/qs" <<'SH'
#!/bin/bash
echo "qs $*" >> "$CALLS"
echo 'quickshell.service.greetd: Received greetd response: {"auth_message":"Password:"}'
echo 'qs: unrelated line'
case "$QS_MODE" in
    launch) echo ready > "$UQ_GREETER_STATUS"; sleep 0.2; echo launched > "$UQ_GREETER_STATUS"; exit 0 ;;
    crash) echo ready > "$UQ_GREETER_STATUS"; exit 134 ;;
    quit) echo ready > "$UQ_GREETER_STATUS"; exit 0 ;;
    hang) sleep 30 ;;
esac
SH
printf '#!/bin/sh\necho "hyprctl $*" >> "%s/calls"\n' "$tmp" > "$tmp/bin/hyprctl"
printf '#!/bin/sh\necho "agreety $*" >> "%s/calls"\n' "$tmp" > "$tmp/bin/agreety"
cat > "$tmp/bin/localectl" <<'SH'
#!/bin/sh
printf '   System Locale: LANG=de_DE.UTF-8\n       VC Keymap: de\n      X11 Layout: %s\n     X11 Variant: %s\n' "$LAYOUT" "$VARIANT"
SH
chmod +x "$tmp/bin/"*

run() { # QS_MODE [extra env]
    local mode=$1
    shift
    env PATH="$tmp/bin:$PATH" CALLS="$tmp/calls" QS_MODE="$mode" XDG_RUNTIME_DIR="$tmp/rt" \
        LAYOUT="${LAYOUT:-de}" VARIANT="${VARIANT:-nodeadkeys}" \
        UQ_GREETER_STATE_DIR="$tmp/state" UQ_GREETER_CONFIG=/usr/share/uq/greeter/hyprland.lua \
        UQ_GREETER_DIR=/usr/share/uq/greeter UQ_GREETER_HYPRLAND="$tmp/bin/Hyprland" \
        UQ_GREETER_HYPRCTL="$tmp/bin/hyprctl" UQ_GREETER_QS="$tmp/bin/qs" UQ_GREETER_AGREETY="$tmp/bin/agreety" \
        UQ_GREETER_MONITORS="${MONITORS:-$tmp/no-monitors.toml}" UQ_GREETER_UQCTL="${UQCTL:-$tmp/bin/no-uqctl}" \
        "$@" "$script" 2>"$tmp/stderr"
}

# 1. A launched session.
: > "$tmp/calls"
run launch; rc=$?
[ "$rc" = 0 ] || { echo "FAIL: launched session exited $rc"; cat "$tmp/stderr"; fail=1; }
[ -s "$tmp/state/failures" ] && { echo "FAIL: a launched session recorded a failure"; fail=1; }
grep -q '^Hyprland --config .*/hyprland.lua$' "$tmp/calls" || { echo "FAIL: Hyprland args: $(cat "$tmp/calls")"; fail=1; }
grep -qx 'qs -p /usr/share/uq/greeter' "$tmp/calls" || { echo "FAIL: quickshell args: $(cat "$tmp/calls")"; fail=1; }
grep -qx "hyprctl dispatch hl.dsp.exit()" "$tmp/calls" || { echo "FAIL: Hyprland not told to exit"; fail=1; }
grep -qx 'require("/usr/share/uq/greeter/hyprland.lua")' "$tmp/last-config.lua" || { echo "FAIL: wrapper doesn't load the fixed config"; cat "$tmp/last-config.lua"; fail=1; }
grep -qx 'hl.config({ input = { kb_layout = "de", kb_variant = "nodeadkeys" } })' "$tmp/last-config.lua" || { echo "FAIL: layout missing"; cat "$tmp/last-config.lua"; fail=1; }
grep -q 'greetd' "$tmp/stderr" && { echo "FAIL: greetd log lines got through: $(cat "$tmp/stderr")"; fail=1; }
grep -q 'unrelated line' "$tmp/stderr" || { echo "FAIL: other quickshell output lost"; fail=1; }
[ -z "$(ls -A "$tmp/rt")" ] || { echo "FAIL: runtime files left: $(ls "$tmp/rt")"; fail=1; }

# 2. The greeter crashes: exit 1, one failure recorded.
: > "$tmp/calls"
run crash; rc=$?
[ "$rc" = 1 ] || { echo "FAIL: crash exited $rc, want 1"; fail=1; }
[ "$(wc -l < "$tmp/state/failures")" = 1 ] || { echo "FAIL: failure not recorded"; fail=1; }
grep -q 'hyprctl dispatch' "$tmp/calls" || { echo "FAIL: Hyprland not told to exit after a crash"; fail=1; }

# 3. Quickshell quits cleanly but no session started: still a failure. That's 2.
run quit; rc=$?
[ "$rc" = 1 ] || { echo "FAIL: quit without a session exited $rc"; fail=1; }

# 4. The third failure in 60 s: agreety on the same VT.
: > "$tmp/calls"
run crash
grep -qx 'agreety --cmd /usr/bin/uq-session' "$tmp/calls" || { echo "FAIL: no agreety after 3 failures: $(cat "$tmp/calls")"; fail=1; }

# 5. The next start goes straight to agreety, without Hyprland.
: > "$tmp/calls"
run launch
grep -q '^Hyprland' "$tmp/calls" && { echo "FAIL: Hyprland started after 3 recent failures"; fail=1; }
grep -qx 'agreety --cmd /usr/bin/uq-session' "$tmp/calls" || { echo "FAIL: no agreety on restart"; fail=1; }

# 6. Old failures don't count; a launch clears the record.
old=$(( $(date +%s) - 120 ))
printf '%s\n%s\n%s\n' "$old" "$old" "$old" > "$tmp/state/failures"
: > "$tmp/calls"
run launch; rc=$?
[ "$rc" = 0 ] || { echo "FAIL: old failures blocked the greeter"; fail=1; }
[ -e "$tmp/state/failures" ] && { echo "FAIL: a launch didn't clear the failures"; fail=1; }

# 7. A greeter that never gets ready is stopped.
: > "$tmp/calls"
start=$(date +%s)
run hang UQ_GREETER_READY_TIMEOUT=1; rc=$?
[ "$rc" = 1 ] || { echo "FAIL: hung greeter exited $rc"; fail=1; }
[ $(( $(date +%s) - start )) -lt 10 ] || { echo "FAIL: hung greeter not stopped in time"; fail=1; }
grep -q 'did not start within' "$tmp/stderr" || { echo "FAIL: no message for the hung greeter"; fail=1; }
rm -f "$tmp/state/failures"

# 8. A hostile layout never reaches the config; the fixed config's "us" stays.
LAYOUT='us" } }) hl.exec_cmd("touch /tmp/pwned") --' run launch
grep -q 'exec_cmd\|kb_layout' "$tmp/last-config.lua" && { echo "FAIL: hostile layout written: $(cat "$tmp/last-config.lua")"; fail=1; }
LAYOUT='us' VARIANT='$(id)' run launch
grep -qx 'hl.config({ input = { kb_layout = "us" } })' "$tmp/last-config.lua" || { echo "FAIL: bad variant handling: $(cat "$tmp/last-config.lua")"; fail=1; }

# 10. The login screen's monitor layout.
grep -q 'hl.monitor' "$tmp/last-config.lua" && { echo "FAIL: monitor lines without a file"; fail=1; }
# A stub uqctl: prints good lines, stray lines, or fails.
cat > "$tmp/bin/uqctl-stub" <<'SH'
#!/bin/sh
[ "$1 $2 $3 $4" = "greeter monitors-lua --file $MONITORS" ] || { echo "bad args: $*" >&2; exit 2; }
case "$UQCTL_MODE" in
    good) printf '%s\n' 'hl.monitor({ output = "DP-1", mode = "1920x1080@60", position = "0x0", scale = "1.0" })' ;;
    stray) printf '%s\n' 'hl.exec_cmd("touch /tmp/pwned")' 'hl.monitor({ output = "DP-2", disabled = true })' ;;
    fail) echo "uqctl: refused" >&2; exit 1 ;;
esac
SH
chmod +x "$tmp/bin/uqctl-stub"
echo '[outputs.DP-1]' > "$tmp/monitors.toml"
MONITORS="$tmp/monitors.toml" UQCTL="$tmp/bin/uqctl-stub" UQCTL_MODE=good run launch
grep -qx 'hl.monitor({ output = "DP-1", mode = "1920x1080@60", position = "0x0", scale = "1.0" })' "$tmp/last-config.lua" \
    || { echo "FAIL: monitor line missing: $(cat "$tmp/last-config.lua") $(cat "$tmp/stderr")"; fail=1; }
[ "$(sed -n 1p "$tmp/last-config.lua")" = 'require("/usr/share/uq/greeter/hyprland.lua")' ] \
    || { echo "FAIL: monitor lines must follow the fixed config"; fail=1; }
MONITORS="$tmp/monitors.toml" UQCTL="$tmp/bin/uqctl-stub" UQCTL_MODE=stray run launch
grep -q 'exec_cmd' "$tmp/last-config.lua" && { echo "FAIL: stray uqctl output reached the config"; fail=1; }
grep -qx 'hl.monitor({ output = "DP-2", disabled = true })' "$tmp/last-config.lua" || { echo "FAIL: good line dropped with the stray one"; fail=1; }
MONITORS="$tmp/monitors.toml" UQCTL="$tmp/bin/uqctl-stub" UQCTL_MODE=fail run launch; rc=$?
[ "$rc" = 0 ] || { echo "FAIL: a refused monitor layout stopped the greeter ($rc)"; fail=1; }
grep -q 'hl.monitor' "$tmp/last-config.lua" && { echo "FAIL: monitor lines after uqctl refused"; fail=1; }
grep -q 'automatic layout' "$tmp/stderr" || { echo "FAIL: no message for the refused layout"; fail=1; }
MONITORS="$tmp/monitors.toml" run launch
grep -q 'hl.monitor' "$tmp/last-config.lua" && { echo "FAIL: monitor lines without uqctl"; fail=1; }
# The real uqctl, when built.
real="$root/target/debug/uqctl"
if [ -x "$real" ]; then
    printf '[outputs.DP-1]\nmode = "2560x1440@144"\nscale = 1.25\n\n[outputs.DP-1.position]\nx = 0\ny = 0\n' > "$tmp/monitors.toml"
    MONITORS="$tmp/monitors.toml" UQCTL="$real" run launch
    grep -qx 'hl.monitor({ output = "DP-1", mode = "2560x1440@144", position = "0x0", scale = "1.25" })' "$tmp/last-config.lua" \
        || { echo "FAIL: real uqctl: $(cat "$tmp/last-config.lua") $(cat "$tmp/stderr")"; fail=1; }
    printf '[outputs."DP-1\\")os.exit()--"]\nmode = "preferred"\n' > "$tmp/monitors.toml"
    MONITORS="$tmp/monitors.toml" UQCTL="$real" run launch
    grep -q 'hl.monitor\|os.exit' "$tmp/last-config.lua" && { echo "FAIL: real uqctl let a bad name through"; fail=1; }
    grep -q 'automatic layout' "$tmp/stderr" || { echo "FAIL: real uqctl: no message for a bad file"; fail=1; }
else
    echo "greeter_session: target/debug/uqctl not built; the real-uqctl monitor checks were skipped"
fi

# 9. --shell without a status path ends Hyprland rather than hanging.
: > "$tmp/calls"
env -u UQ_GREETER_STATUS PATH="$tmp/bin:$PATH" UQ_GREETER_HYPRCTL="$tmp/bin/hyprctl" UQ_GREETER_QS="$tmp/bin/qs" \
    "$script" --shell 2>/dev/null
grep -q 'hyprctl dispatch' "$tmp/calls" || { echo "FAIL: --shell without status didn't exit Hyprland"; fail=1; }

[ "$fail" = 0 ] && echo "greeter_session ok"
exit "$fail"
