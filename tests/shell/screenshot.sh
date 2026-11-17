#!/usr/bin/env bash
# packaging/uq-screenshot with stub grim, slurp, wl-copy, notify-send, hyprctl and qs
# (UQ_SHOT_* overrides, test-only):
#   - region: slurp picks the area first, grim captures just that (no full-screen capture, no
#     editor window); Esc in slurp captures nothing and is not an error; a slurp that fails for
#     another reason is reported
#   - every capture is saved to the dated file, copied, and handed to the shell's preview over
#     `qs ipc call screenshot present <file> copied`; no notification when the preview showed it
#   - without a shell (qs fails) a notification names the file, with no "Open" action
#   - print: follows [ui] print_action in state.toml (region when unset or unknown)
#   - a second run while one is being taken does nothing, but a wl-copy still serving the
#     clipboard does not block the next capture (it must not inherit the lock)
#   - window: grim -g with the focused window's geometry; with no focused window, exit 1
#   - screen: grim -o the focused output
#   - anything else: usage, exit 2
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
script="$root/packaging/uq-screenshot"
[ -x "$script" ] || { echo "FAIL: packaging/uq-screenshot missing or not executable"; exit 1; }
command -v jq >/dev/null || { echo "screenshot skipped (no jq)"; exit 0; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fail=0
bad() { echo "FAIL: $*"; fail=1; }
mkdir -p "$tmp/bin" "$tmp/out"
calls="$tmp/calls"

cat > "$tmp/bin/grim" <<SH
#!/bin/bash
echo "grim \$*" >> "$calls"
printf 'png' > "\${@: -1}"
SH
printf '#!/bin/bash\necho "slurp" >> "%s"\necho "10,20 30x40"\n' "$calls" > "$tmp/bin/slurp"
# wl-copy forks to serve the clipboard: a background process that outlives it and keeps whatever
# file descriptors it inherited.
cat > "$tmp/bin/wl-copy" <<SH
#!/bin/bash
echo "wl-copy \$*" >> "$calls"
cat >/dev/null
[ "\${NOCOPY:-}" = 1 ] && exit 1
sleep 5 &
SH
printf '#!/bin/bash\necho "notify $*" >> "%s"\n' "$calls" > "$tmp/bin/notify-send"
cat > "$tmp/bin/qs" <<SH
#!/bin/bash
echo "qs \$*" >> "$calls"
[ "\${NOSHELL:-}" = 1 ] && exit 1
echo ok
SH
cat > "$tmp/bin/hyprctl" <<'SH'
#!/bin/bash
case "$1" in
monitors) echo '[{"name":"HDMI-A-1","focused":false},{"name":"eDP-1","focused":true}]' ;;
activewindow) [ "${NOWIN:-}" = 1 ] && echo '{}' || echo '{"at":[14,74],"size":[939,992]}' ;;
esac
SH
chmod +x "$tmp/bin/"*

run() { # mode [env...]
    local mode=$1; shift
    : > "$calls"; rm -f "$tmp/out/"*
    env "$@" UQ_SHOT_DIR="$tmp/out" UQ_SHOT_GRIM="$tmp/bin/grim" \
        UQ_SHOT_SLURP="$tmp/bin/slurp" UQ_SHOT_COPY="$tmp/bin/wl-copy" UQ_SHOT_NOTIFY="$tmp/bin/notify-send" \
        UQ_SHOT_HYPRCTL="$tmp/bin/hyprctl" UQ_SHOT_QS="$tmp/bin/qs" UQ_SHOT_SHELL=/shell \
        UQ_SHOT_STATE="$tmp/state.toml" XDG_RUNTIME_DIR="$tmp" \
        "$script" "$mode" 2>"$tmp/stderr" >/dev/null
}
saved() { compgen -G "$tmp/out/Screenshot_[0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]_[0-9][0-9][0-9][0-9][0-9][0-9].png" >/dev/null; }

# --- region -------------------------------------------------------------------------------------
run region; rc=$?
[ "$rc" = 0 ] || bad "region exited $rc: $(cat "$tmp/stderr")"
grep -q '^slurp' "$calls" || bad "region: no slurp: $(cat "$calls")"
grep -q '^grim -g 10,20 30x40 ' "$calls" || bad "region: grim not on the picked area: $(cat "$calls")"
grep -q '^grim -o' "$calls" && bad "region: a full-screen capture was taken first"
saved || bad "region: nothing saved"
grep -q '^wl-copy --type image/png' "$calls" || bad "region: not copied"
grep -q '^qs ipc -p /shell call screenshot present .*/Screenshot_[0-9_]*\.png copied$' "$calls" || bad "region: the preview wasn't asked to show it: $(cat "$calls")"
grep -q '^notify' "$calls" && bad "region: notified although the preview showed it"
grep -q 'satty\|xdg-open' "$calls" && bad "region: opened a viewer"
compgen -G "$tmp/uq-shot.*" >/dev/null && bad "region: scratch files were left behind"

# Esc in slurp: nothing captured, no error.
printf '#!/bin/bash\necho "slurp" >> "%s"\necho "selection cancelled" >&2\nexit 1\n' "$calls" > "$tmp/bin/slurp"
run region; rc=$?
[ "$rc" = 0 ] || bad "cancelled region exited $rc"
grep -q '^grim' "$calls" && bad "cancelled region still captured"
grep -q '^notify' "$calls" && bad "cancelled region notified"
# slurp failing for another reason is reported.
printf '#!/bin/bash\necho "slurp" >> "%s"\necho "compositor doesn'"'"'t support layer shell" >&2\nexit 1\n' "$calls" > "$tmp/bin/slurp"
run region; rc=$?
[ "$rc" = 1 ] || bad "a broken slurp exited $rc"
grep -q '^notify .*Screenshot failed' "$calls" || bad "a broken slurp wasn't reported: $(cat "$calls")"
printf '#!/bin/bash\necho "slurp" >> "%s"\necho "10,20 30x40"\n' "$calls" > "$tmp/bin/slurp"

# --- print follows the setting ------------------------------------------------------------------
run print; rc=$?
grep -q '^slurp' "$calls" || bad "print with no setting isn't region: $(cat "$calls")"
printf '[settings]\nx = 1\n[ui]\nreduce_motion = false\nprint_action = "screen"\n[other]\nprint_action = "window"\n' > "$tmp/state.toml"
run print; rc=$?
grep -q '^grim -o eDP-1 ' "$calls" || bad "print_action screen: $(cat "$calls")"
printf '[ui]\nprint_action = "window"\n' > "$tmp/state.toml"
run ""; rc=$?
grep -q '^grim -g 14,74 939x992 ' "$calls" || bad "print_action window (no argument): $(cat "$calls")"
printf '[ui]\nprint_action = "bogus"\n' > "$tmp/state.toml"
run print; rc=$?
grep -q '^slurp' "$calls" || bad "unknown print_action isn't region"
rm -f "$tmp/state.toml"

# --- one capture at a time ----------------------------------------------------------------------
if command -v flock >/dev/null; then
    ( exec 9>"$tmp/uq-screenshot.lock"; flock -n 9 || exit 1
      run region; echo "rc=$?" > "$tmp/busy_rc" )
    [ "$(cat "$tmp/busy_rc")" = rc=0 ] || bad "a busy run failed"
    [ ! -s "$calls" ] || bad "a busy run still captured: $(cat "$calls")"
    # The wl-copy of the last capture is still running (it sleeps 5 s): that must not count as a
    # capture in progress.
    run window; rc=$?
    [ "$rc" = 0 ] && grep -q '^grim' "$calls" || bad "a wl-copy still serving the clipboard blocked the next capture: $(cat "$calls")"
    run window; rc=$?
    grep -q '^grim' "$calls" || bad "the capture after that was blocked too"
fi

# --- no shell: a notification names the file, with no action ------------------------------------
NOSHELL=1 run window; rc=$?
[ "$rc" = 0 ] || bad "window without a shell exited $rc"
grep -q '^notify .*Screenshot saved.*Screenshot_' "$calls" || bad "no shell: no notification naming the file: $(cat "$calls")"
grep -q -- '--action' "$calls" && bad "no shell: the notification has an action"

# --- no wl-copy: still saved and shown, but not marked copied -----------------------------------
NOCOPY=1 run window; rc=$?
grep -q '^qs ipc .* call screenshot present .*\.png none$' "$calls" || bad "copy failed: the preview was told it was copied: $(cat "$calls")"
saved || bad "copy failed: nothing saved"

# --- window / screen / usage --------------------------------------------------------------------
run window; rc=$?
[ "$rc" = 0 ] || bad "window exited $rc"
grep -q '^grim -g 14,74 939x992 ' "$calls" || bad "window: $(cat "$calls")"
saved || bad "window: nothing saved"

NOWIN=1 run window; rc=$?
[ "$rc" = 1 ] || bad "window with none focused exited $rc"

run screen; rc=$?
[ "$rc" = 0 ] || bad "screen exited $rc"
grep -q '^grim -o eDP-1 ' "$calls" || bad "screen: $(cat "$calls")"
grep -q '^wl-copy --type image/png' "$calls" || bad "screen: not copied"

run nonsense; rc=$?
[ "$rc" = 2 ] || bad "unknown mode exited $rc"

[ "$fail" = 0 ] && echo "screenshot ok"
exit "$fail"
