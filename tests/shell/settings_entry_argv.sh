#!/usr/bin/env bash
# Quick settings' "Settings…" entry must run exactly ["uqctl","settings"] (a fixed argv, no
# shell string) and close the panel.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
f="$root/shell/quick/SettingsEntry.qml"
fail=0
[ -f "$f" ] || { echo "FAIL: $f missing"; exit 1; }
n=$(grep -c 'execDetached(\["uqctl", "settings"\])' "$f")
[ "$n" = 1 ] || { echo "FAIL: expected exactly one execDetached([\"uqctl\", \"settings\"]) in SettingsEntry.qml, found $n"; fail=1; }
if grep -n 'execDetached' "$f" | grep -v 'execDetached(\["uqctl", "settings"\])'; then
    echo "FAIL: SettingsEntry.qml runs another command"; fail=1
fi
grep -q 'Surfaces.close()' "$f" || { echo "FAIL: SettingsEntry.qml does not close the panel"; fail=1; }
grep -q 'Accessible.name' "$f" || { echo "FAIL: SettingsEntry.qml has no Accessible.name"; fail=1; }
grep -q 'SettingsEntry' "$root/shell/quick/QuickSettings.qml" || { echo "FAIL: QuickSettings.qml does not use SettingsEntry"; fail=1; }
[ "$fail" = 0 ] && echo "settings_entry_argv: ok"
exit "$fail"
