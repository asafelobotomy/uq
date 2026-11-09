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

types='(Text|Label|TextEdit|TextInput)'
qual='([A-Za-z_][A-Za-z0-9_]*\.)*'
files=$(find shell lock -type f \( -name '*.qml' -o -name '*.js' \) ! -path 'shell/core/UqText.qml' ! -path 'shell/core/UqInput.qml' \
    ! -path 'shell/notif/builtin/NotifBody.qml' 2>/dev/null | sort)
[ -z "$files" ] && exit 0

hits=$(
    # shellcheck disable=SC2086
    {
        # Type followed by `{` on the same line, anywhere on the line.
        grep -nHE "(^|[^A-Za-z0-9_.])${qual}${types}[[:space:]]*\{" $files
        # Type at the end of a line with `{` opening the next non-blank line.
        RE="(^|[^A-Za-z0-9_.])${qual}${types}[[:space:]]*\$" awk '
            BEGIN { re = ENVIRON["RE"] }
            FNR == 1 { prev = "" }
            prev != "" && $0 ~ /^[[:space:]]*\{/ { print FILENAME ":" prevn ":" prev; }
            $0 ~ /^[[:space:]]*$/ { next }
            { prev = ""; if ($0 ~ re) { prev = $0; prevn = FNR } }
        ' $files
        grep -nHE 'Qt\.createQmlObject' $files
    } | sort -t: -k1,1 -k2,2n -u
)
if [ -n "$hits" ]; then
    echo "lint-shell: Text/Label/TextEdit/TextInput objects and Qt.createQmlObject are forbidden" >&2
    echo "in shell/ and lock/ outside shell/core/UqText.qml (and TextInput in shell/core/UqInput.qml, StyledText" >&2
    echo "in shell/notif/builtin/NotifBody.qml); use UqText:" >&2
    echo "$hits" >&2
    exit 1
fi
