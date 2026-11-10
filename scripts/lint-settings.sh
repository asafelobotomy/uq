#!/usr/bin/env bash
# Fails (exit 1, listing file:line) if any .qml/.js under apps/uq-settings/qml, other than
# qml/components/UqLabel.qml, instantiates Text, Label, TextEdit or TextInput, or calls
# Qt.createQmlObject. All Settings text goes through UqLabel (plain text only; spec §2.1).
# It also fails on any word of scripts/settings-jargon.txt inside a quoted string (plain words).
set -u
cd "$(dirname "$0")/.." || exit 2
# shellcheck source=scripts/lib-qml-lint.sh
. scripts/lib-qml-lint.sh

dir=apps/uq-settings/qml
[ -d "$dir" ] || exit 0
mapfile -t files < <(find "$dir" -type f \( -name '*.qml' -o -name '*.js' \) ! -path "$dir/components/UqLabel.qml" | sort)
[ ${#files[@]} = 0 ] && exit 0

hits=$(qml_lint_hits "${files[@]}")
if [ -n "$hits" ]; then
    echo "lint-settings: Text/Label/TextEdit/TextInput objects and Qt.createQmlObject are forbidden" >&2
    echo "in $dir outside components/UqLabel.qml; use UqLabel:" >&2
    echo "$hits" >&2
    exit 1
fi

hits=$(awk -f scripts/lint-control-text.awk "${files[@]}" | sort -t: -k1,1 -k2,2n -u)
if [ -n "$hits" ]; then
    echo "lint-settings: stock controls must show data through UqLabel contentItems" >&2
    echo "(text: may only be a string literal; a ComboBox needs contentItem: and delegate:):" >&2
    echo "$hits" >&2
    exit 1
fi

hits=$(awk -f scripts/lint-jargon.awk scripts/settings-jargon.txt "${files[@]}")
if [ -n "$hits" ]; then
    echo "lint-settings: technical words in text people read; say it in plain language" >&2
    echo "(or end the line with '// jargon-ok: <reason>'; words: scripts/settings-jargon.txt):" >&2
    echo "$hits" >&2
    exit 1
fi
