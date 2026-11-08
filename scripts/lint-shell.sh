#!/usr/bin/env bash
# Fails (exit 1, listing file:line) if shell/ contains a bare `Text {` outside shell/core/UqText.qml.
# All shell text must go through UqText (plain text only; spec §2.10).
set -u
cd "$(dirname "$0")/.." || exit 2
hits=$(grep -rnE '^\s*Text\s*\{' shell/ | grep -v '^shell/core/UqText\.qml:' || true)
if [ -n "$hits" ]; then
    echo "lint-shell: bare Text { is forbidden outside shell/core/UqText.qml; use UqText:" >&2
    echo "$hits" >&2
    exit 1
fi
