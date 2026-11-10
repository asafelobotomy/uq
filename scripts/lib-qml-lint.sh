# shellcheck shell=bash
# Sourced by lint-shell.sh and lint-settings.sh. `qml_lint_hits FILE...` prints file:line:text for
# every Text, Label, TextEdit or TextInput object (qualified, aliased, inline, or with the brace on
# the next line) and every Qt.createQmlObject call in the files, sorted and de-duplicated.
# QtQuick.Controls Label defaults to AutoText, so it is as dangerous as a bare Text.
qml_lint_hits() {
    local types='(Text|Label|TextEdit|TextInput)'
    local qual='([A-Za-z_][A-Za-z0-9_]*\.)*'
    {
        # Type followed by `{` on the same line, anywhere on the line.
        grep -nHE "(^|[^A-Za-z0-9_.])${qual}${types}[[:space:]]*\{" "$@"
        # Type at the end of a line with `{` opening the next non-blank line.
        RE="(^|[^A-Za-z0-9_.])${qual}${types}[[:space:]]*\$" awk '
            BEGIN { re = ENVIRON["RE"] }
            FNR == 1 { prev = "" }
            prev != "" && $0 ~ /^[[:space:]]*\{/ { print FILENAME ":" prevn ":" prev; }
            $0 ~ /^[[:space:]]*$/ { next }
            { prev = ""; if ($0 ~ re) { prev = $0; prevn = FNR } }
        ' "$@"
        grep -nHE 'Qt\.createQmlObject' "$@"
    } | sort -t: -k1,1 -k2,2n -u
}
