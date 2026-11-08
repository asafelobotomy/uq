#!/usr/bin/env bash
# lint-shell.sh must fail on a bare Text { outside UqText.qml and pass otherwise.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/scripts" "$tmp/shell/core"
cp "$root/scripts/lint-shell.sh" "$tmp/scripts/"
printf 'Text {\n  text: "x"\n}\n' > "$tmp/shell/core/UqText.qml"
printf 'Item {\n}\n' > "$tmp/shell/Ok.qml"
(cd "$tmp" && scripts/lint-shell.sh >/dev/null) || { echo "FAIL: clean tree rejected"; exit 1; }
printf 'Item {\n    Text {\n    }\n}\n' > "$tmp/shell/Bad.qml"
out=$(cd "$tmp" && scripts/lint-shell.sh 2>&1) && { echo "FAIL: bare Text accepted"; exit 1; }
case "$out" in *shell/Bad.qml:2*) ;; *) echo "FAIL: no file:line in: $out"; exit 1;; esac
echo "lint_test ok"
