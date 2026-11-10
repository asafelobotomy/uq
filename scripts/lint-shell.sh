#!/usr/bin/env bash
# Fails (exit 1, listing file:line) if any .qml/.js under shell/ or lock/, other than shell/core/UqText.qml,
# shell/core/UqInput.qml and shell/notif/builtin/NotifBody.qml,
#   - instantiates Text, Label, TextEdit or TextInput (qualified, aliased, inline, or with the
#     brace on the next line), or
#   - calls Qt.createQmlObject.
# All shell text must go through UqText (plain text only; spec §2.10). QtQuick.Controls Label
# defaults to AutoText, so it is as dangerous as a bare Text. NotifBody.qml is the one StyledText:
# it renders only logic/sanitize.js output (spec §2.5).
set -u
cd "$(dirname "$0")/.." || exit 2

# shellcheck source=scripts/lib-qml-lint.sh
. scripts/lib-qml-lint.sh

dirs=()
for d in shell lock; do [ -d "$d" ] && dirs+=("$d"); done
[ ${#dirs[@]} = 0 ] && exit 0
mapfile -t files < <(find "${dirs[@]}" -type f \( -name '*.qml' -o -name '*.js' \) ! -path 'shell/core/UqText.qml' \
    ! -path 'shell/core/UqInput.qml' ! -path 'shell/notif/builtin/NotifBody.qml' | sort)
[ ${#files[@]} = 0 ] && exit 0

hits=$(qml_lint_hits "${files[@]}")
if [ -n "$hits" ]; then
    echo "lint-shell: Text/Label/TextEdit/TextInput objects and Qt.createQmlObject are forbidden" >&2
    echo "in shell/ and lock/ outside shell/core/UqText.qml (and TextInput in shell/core/UqInput.qml, StyledText" >&2
    echo "in shell/notif/builtin/NotifBody.qml); use UqText:" >&2
    echo "$hits" >&2
    exit 1
fi
