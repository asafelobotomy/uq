#!/usr/bin/env bash
# packaging/uq-session with a temporary $HOME and stub uqctl, start-hyprland and Hyprland on PATH:
#   - first run (no hyprland.lua): runs `uqctl setup --quiet`, then execs the compositor
#   - second run (the stub is in place): doesn't run setup again
#   - a failing setup is written to ~/.local/state/uq/session.log and the session still starts
#   - start-hyprland is preferred; plain Hyprland when start-hyprland isn't installed
# It never starts a real compositor.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
script="$root/packaging/uq-session"
[ -x "$script" ] || { echo "FAIL: packaging/uq-session missing or not executable"; exit 1; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fail=0

home="$tmp/home"
mkdir -p "$home" "$tmp/bin"
# uqctl stub: logs its args; writes the stub line unless UQCTL_FAIL is set.
cat > "$tmp/bin/uqctl" <<'SH'
#!/bin/sh
echo "uqctl $*" >> "$CALLS"
if [ -n "${UQCTL_FAIL:-}" ]; then
    echo "uqctl: uQ's base config /usr/share/uq/hypr/base.lua is missing; is the uq package installed?" >&2
    exit 1
fi
mkdir -p "$HOME/.config/hypr"
printf 'require("/usr/share/uq/hypr/base.lua")\nrequire("./uq/generated.lua")\nrequire("./uq/user.lua")\n' > "$HOME/.config/hypr/hyprland.lua"
SH
for c in start-hyprland Hyprland; do
    printf '#!/bin/sh\necho "%s $$ $*" >> "$CALLS"\n' "$c" > "$tmp/bin/$c"
done
chmod +x "$tmp/bin/"*

run() { # extra env...
    env -i HOME="$home" PATH="$tmp/bin:/usr/bin:/bin" CALLS="$tmp/calls" "$@" "$script"
}

# 1. First login: setup runs, then start-hyprland.
: > "$tmp/calls"
run; rc=$?
[ "$rc" = 0 ] || { echo "FAIL: first run exited $rc"; fail=1; }
grep -qx 'uqctl setup --quiet' "$tmp/calls" || { echo "FAIL: setup not run on first login: $(cat "$tmp/calls")"; fail=1; }
grep -q '^start-hyprland ' "$tmp/calls" || { echo "FAIL: session not started: $(cat "$tmp/calls")"; fail=1; }
[ "$(head -n1 "$tmp/calls")" = 'uqctl setup --quiet' ] || { echo "FAIL: setup must run before the compositor"; fail=1; }
[ -e "$home/.local/state/uq/session.log" ] && { echo "FAIL: a good setup wrote the log"; fail=1; }

# 2. Second login: the stub is there, so no setup.
: > "$tmp/calls"
run
grep -q '^uqctl' "$tmp/calls" && { echo "FAIL: setup ran again: $(cat "$tmp/calls")"; fail=1; }
grep -q '^start-hyprland ' "$tmp/calls" || { echo "FAIL: second run did not start the session"; fail=1; }

# 3. exec: the compositor replaces the script (same PID).
: > "$tmp/calls"
env -i HOME="$home" PATH="$tmp/bin:/usr/bin:/bin" CALLS="$tmp/calls" "$script" &
pid=$!
wait "$pid"
grep -q "^start-hyprland $pid\$\|^start-hyprland $pid " "$tmp/calls" || { echo "FAIL: start-hyprland is not exec'd (pid $pid): $(cat "$tmp/calls")"; fail=1; }

# 4. A user's own hyprland.lua without the stub line: setup is asked (and keeps the file).
printf -- '-- mine\n' > "$home/.config/hypr/hyprland.lua"
: > "$tmp/calls"
run UQCTL_FAIL=1
grep -qx 'uqctl setup --quiet' "$tmp/calls" || { echo "FAIL: setup not run for a config without the stub"; fail=1; }

# 5. Failing setup: logged, and the session still starts.
rm -rf "$home/.config" "$home/.local"
: > "$tmp/calls"
run UQCTL_FAIL=1; rc=$?
[ "$rc" = 0 ] || { echo "FAIL: failing setup changed the exit ($rc)"; fail=1; }
grep -q '^start-hyprland ' "$tmp/calls" || { echo "FAIL: failing setup kept the session from starting"; fail=1; }
log="$home/.local/state/uq/session.log"
grep -q 'uqctl setup exited 1' "$log" 2>/dev/null || { echo "FAIL: setup failure not logged: $(cat "$log" 2>&1)"; fail=1; }
grep -q 'base config' "$log" 2>/dev/null || { echo "FAIL: setup's message not logged"; fail=1; }
# XDG_STATE_HOME is honoured.
rm -rf "$home/.local"
env -i HOME="$home" XDG_STATE_HOME="$tmp/state" PATH="$tmp/bin:/usr/bin:/bin" CALLS="$tmp/calls" UQCTL_FAIL=1 "$script"
grep -q 'uqctl setup exited 1' "$tmp/state/uq/session.log" 2>/dev/null || { echo "FAIL: XDG_STATE_HOME ignored"; fail=1; }

# 6. uqctl missing entirely: logged, session still starts.
mkdir -p "$tmp/bin2"
cp "$tmp/bin/start-hyprland" "$tmp/bin2/"
rm -rf "$home/.config" "$home/.local"
: > "$tmp/calls"
env -i HOME="$home" PATH="$tmp/bin2:/usr/bin:/bin" CALLS="$tmp/calls" "$script"
grep -q '^start-hyprland ' "$tmp/calls" || { echo "FAIL: missing uqctl kept the session from starting"; fail=1; }
grep -q 'uqctl setup exited' "$home/.local/state/uq/session.log" 2>/dev/null || { echo "FAIL: missing uqctl not logged"; fail=1; }

# 7. No start-hyprland: plain Hyprland. A PATH of only the tools the script needs.
mkdir -p "$tmp/min"
for t in bash sh grep mkdir date wc tail mv head cat; do
    p=$(command -v "$t") && ln -sf "$p" "$tmp/min/$t"
done
cp "$tmp/bin/Hyprland" "$tmp/bin/uqctl" "$tmp/min/"
rm -rf "$home/.config" "$home/.local"
: > "$tmp/calls"
env -i HOME="$home" PATH="$tmp/min" CALLS="$tmp/calls" "$tmp/min/bash" "$script"
grep -q '^Hyprland ' "$tmp/calls" || { echo "FAIL: no fallback to Hyprland: $(cat "$tmp/calls")"; fail=1; }

[ "$fail" = 0 ] && echo "uq_session ok"
exit "$fail"
