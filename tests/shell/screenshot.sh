#!/usr/bin/env bash
# packaging/uq-screenshot with stub grim, satty, slurp, wl-copy, notify-send and hyprctl
# (UQ_SHOT_* overrides, test-only):
#   - region: grim captures the focused output, satty opens it full screen with the crop tool,
#     Enter copying and saving to the dated file; without satty, slurp picks the area
#   - window: grim -g with the focused window's geometry; with no focused window, exit 1
#   - screen: grim -o the focused output; the file is copied and announced
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
cat > "$tmp/bin/satty" <<SH
#!/bin/bash
echo "satty \$*" >> "$calls"
# Pressing Enter: satty writes the output file.
while [ \$# -gt 0 ]; do [ "\$1" = --output-filename ] && printf 'png' > "\$2"; shift; done
SH
printf '#!/bin/bash\necho "slurp" >> "%s"\necho "10,20 30x40"\n' "$calls" > "$tmp/bin/slurp"
printf '#!/bin/bash\necho "wl-copy $*" >> "%s"\ncat >/dev/null\n' "$calls" > "$tmp/bin/wl-copy"
printf '#!/bin/bash\necho "notify $*" >> "%s"\n' "$calls" > "$tmp/bin/notify-send"
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
    env "$@" UQ_SHOT_DIR="$tmp/out" UQ_SHOT_GRIM="$tmp/bin/grim" UQ_SHOT_SATTY="${SATTY:-$tmp/bin/satty}" \
        UQ_SHOT_SLURP="$tmp/bin/slurp" UQ_SHOT_COPY="$tmp/bin/wl-copy" UQ_SHOT_NOTIFY="$tmp/bin/notify-send" \
        UQ_SHOT_HYPRCTL="$tmp/bin/hyprctl" XDG_RUNTIME_DIR="$tmp" "$script" "$mode" 2>"$tmp/stderr"
}
saved() { compgen -G "$tmp/out/Screenshot_[0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]_[0-9][0-9][0-9][0-9][0-9][0-9].png" >/dev/null; }

run region; rc=$?; wait
[ "$rc" = 0 ] || bad "region exited $rc: $(cat "$tmp/stderr")"
grep -q '^grim -o eDP-1 ' "$calls" || bad "region: grim not on the focused output: $(cat "$calls")"
grep -q -- '--fullscreen --initial-tool crop' "$calls" || bad "region: satty args: $(cat "$calls")"
grep -q -- '--actions-on-enter save-to-clipboard,save-to-file,exit' "$calls" || bad "region: Enter doesn't copy and save"
saved || bad "region: nothing saved"
compgen -G "$tmp/uq-shot.*" >/dev/null && bad "region: the capture was left behind"

SATTY="$tmp/bin/missing-satty" run region; rc=$?; wait
[ "$rc" = 0 ] || bad "region without satty exited $rc"
grep -q '^slurp' "$calls" || bad "region without satty: no slurp: $(cat "$calls")"
grep -q '^grim -g 10,20 30x40 ' "$calls" || bad "region without satty: $(cat "$calls")"
grep -q '^wl-copy --type image/png' "$calls" || bad "region without satty: not copied"

run window; rc=$?; wait
[ "$rc" = 0 ] || bad "window exited $rc"
grep -q '^grim -g 14,74 939x992 ' "$calls" || bad "window: $(cat "$calls")"
saved || bad "window: nothing saved"
grep -q '^notify .*Screenshot saved and copied' "$calls" || bad "window: no notification: $(cat "$calls")"

NOWIN=1 run window; rc=$?; wait
[ "$rc" = 1 ] || bad "window with none focused exited $rc"

run screen; rc=$?; wait
[ "$rc" = 0 ] || bad "screen exited $rc"
grep -q '^grim -o eDP-1 ' "$calls" || bad "screen: $(cat "$calls")"
grep -q '^wl-copy --type image/png' "$calls" || bad "screen: not copied"

run nonsense; rc=$?
[ "$rc" = 2 ] || bad "unknown mode exited $rc"

[ "$fail" = 0 ] && echo "screenshot ok"
exit "$fail"
