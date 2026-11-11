#!/usr/bin/env bash
# The settings page template and its test kit (templates/settings-page/,
# templates/settings-page-tests/; docs/settings-pages.md "Testing a page"):
#   1. every property and signal of each stub component exists on the real org.uq.settings
#      component it stands in for, so a page tested against the stub works in Settings;
#   2. the template page passes its offscreen test against the stub (skipped without
#      qmltestrunner);
#   3. the template, laid out as an app's uq/ folder, passes `uqctl integrations check
#      --no-programs`.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
stub=$root/templates/settings-page-tests/stub/org/uq/settings
real=$root/apps/uq-settings/qml/components
fail=0
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
bad() { echo "FAIL: $*"; fail=1; }

# --- 1. the stub's API is a subset of the real one ---
members() { sed -nE 's/^ *(readonly +)?(default +)?property +[A-Za-z<>]+ +([A-Za-z_][A-Za-z0-9_]*).*/\3/p; s/^ *signal +([A-Za-z_][A-Za-z0-9_]*).*/\1/p' "$@"; }
while read -r name _ file; do
    case "$name" in module|'') continue ;; esac
    [ -f "$real/$file" ] || { bad "stub $file has no real component"; continue; }
    known=$(members "$real/$file"; grep -q '^SettingRow' "$real/$file" && members "$real/SettingRow.qml")
    for m in $(members "$stub/$file"); do
        grep -qx "$m" <<<"$known" || bad "stub $file: $m is not on the real $name"
    done
done <"$stub/qmldir"

# --- 2. the template's own test ---
qtr=${QMLTESTRUNNER:-}
[ -z "$qtr" ] && for c in /usr/lib/qt6/bin/qmltestrunner qmltestrunner6 qmltestrunner; do
    command -v "$c" >/dev/null && { qtr=$c; break; }
done
if [ -n "$qtr" ]; then
    QT_QPA_PLATFORM=offscreen "$qtr" -import "$root/templates/settings-page-tests/stub" \
        -input "$root/templates/settings-page-tests" >"$tmp/qml.log" 2>&1 ||
        bad "the template's QML test failed: $(grep -E 'FAIL|Totals' "$tmp/qml.log")"
else
    echo "SKIP: no qmltestrunner for the template's QML test"
fi

# --- 3. the template passes the check, without its program installed ---
uqctl=${UQCTL:-$root/target/debug/uqctl}
if [ ! -x "$uqctl" ]; then
    (cd "$root" && cargo build -q -p uqctl) || { echo "FAIL: couldn't build uqctl"; exit 1; }
fi
mkdir -p "$tmp/uq/pages"
cp -r "$root/templates/settings-page" "$tmp/uq/pages/example"
out=$("$uqctl" integrations check --no-programs "$tmp/uq") || bad "check failed on the template: $out"

[ "$fail" = 0 ] && echo "ok: settings page template"
exit "$fail"
