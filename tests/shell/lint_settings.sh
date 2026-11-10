#!/usr/bin/env bash
# lint-settings.sh must fail on Text/Label/TextEdit/TextInput (and createQmlObject) anywhere under
# apps/uq-settings/qml except components/UqLabel.qml, and pass on UqLabel.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
qml="$tmp/apps/uq-settings/qml"
mkdir -p "$tmp/scripts" "$qml/components" "$qml/pages"
cp "$root/scripts/lint-settings.sh" "$root/scripts/lib-qml-lint.sh" "$tmp/scripts/" || exit 1
printf 'import QtQuick.Controls as Controls\nControls.Label {\n  textFormat: Text.PlainText\n}\n' > "$qml/components/UqLabel.qml"
fail=0

run() { (cd "$tmp" && scripts/lint-settings.sh 2>&1); }

bad() { # name, file (under qml/), content, expected line
    printf '%b' "$3" > "$qml/$2"
    if out=$(run); then echo "FAIL (accepted): $1"; fail=1
    else case "$out" in *"apps/uq-settings/qml/$2:$4"*) ;; *) echo "FAIL ($1): no file:line $4 in: $out"; fail=1;; esac; fi
    rm -f "$qml/$2"
}
good() { # name, content
    printf '%b' "$2" > "$qml/Case.qml"
    if ! out=$(run); then echo "FAIL (rejected): $1: $out"; fail=1; fi
    rm -f "$qml/Case.qml"
}

good empty 'Item {\n}\n'
good uqlabel 'Item {\n  UqLabel { text: "x" }\n}\n'
good enums 'UqLabel {\n  elide: Text.ElideRight\n}\n'

bad bare Case.qml 'Item {\n    Text {}\n}\n' 2
bad label Case.qml 'Item {\n    Controls.Label { }\n}\n' 2
bad kirigami pages/P.qml 'Item {\n    QQC2.Label\n    {\n    }\n}\n' 2
bad textedit components/Row.qml 'Item {\n    TextEdit { }\n}\n' 2
bad textinput Case.qml 'Item { TextInput { } }\n' 1
bad createqml Case.qml 'Item {\n    Component.onCompleted: Qt.createQmlObject("x", this)\n}\n' 2

# Only components/UqLabel.qml is exempt, not a file of that name elsewhere.
bad elsewhere pages/UqLabel.qml 'Text {\n}\n' 1

[ $fail = 0 ] && echo "lint_settings ok"
exit $fail
