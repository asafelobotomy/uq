#!/usr/bin/env bash
# lint-shell.sh must fail on every form of Text/Label/TextEdit/TextInput (and createQmlObject)
# outside UqText.qml, and pass on the allowed forms.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/scripts" "$tmp/shell/core"
cp "$root/scripts/lint-shell.sh" "$tmp/scripts/"
printf 'Text {\n  text: "x"\n}\n' > "$tmp/shell/core/UqText.qml"
fail=0

run() { (cd "$tmp" && scripts/lint-shell.sh 2>&1); }

bad() { # name, content, expected line
    printf '%b' "$2" > "$tmp/shell/Case.qml"
    if out=$(run); then echo "FAIL (accepted): $1"; fail=1
    else case "$out" in *"shell/Case.qml:$3"*) ;; *) echo "FAIL ($1): no file:line $3 in: $out"; fail=1;; esac; fi
    rm -f "$tmp/shell/Case.qml"
}
good() { # name, content
    printf '%b' "$2" > "$tmp/shell/Case.qml"
    if ! out=$(run); then echo "FAIL (rejected): $1: $out"; fail=1; fi
    rm -f "$tmp/shell/Case.qml"
}

good empty 'Item {\n}\n'
good UqText 'Item {\n  UqText { text: "x" }\n}\n'
good enums 'UqText {\n  textFormat: Text.PlainText\n  elide: Text.ElideRight\n}\n'
good props 'Item {\n  property string text: "a"\n  property string label: "Text"\n}\n'
good identifiers 'Item {\n  MyText { }\n  TextMetrics { }\n  LabelHelper { }\n}\n'

bad bare 'Item {\n    Text {\n    }\n}\n' 2
bad nospace 'Item {\n    Text{\n    }\n}\n' 2
bad qualified 'Item {\n    QQ.Text {\n    }\n}\n' 2
bad label 'Item {\n    Controls.Label {\n    }\n}\n' 2
bad plainlabel 'Item {\n    Label { }\n}\n' 2
bad textedit 'Item {\n    TextEdit { }\n}\n' 2
bad textinput 'Item {\n    TextInput { }\n}\n' 2
bad inline 'Item { Text { } }\n' 1
bad delegate 'ListView {\n    delegate: Text { text: x }\n}\n' 2
bad component 'Item {\n    Component { Text {\n    } }\n}\n' 2
bad nextline 'Item {\n    Text\n    {\n    }\n}\n' 2
bad nextline_prop 'ListView {\n    delegate: Label\n\n    {\n    }\n}\n' 2
bad deepqual 'Item {\n    A.B.Text { }\n}\n' 2
bad createqml 'Item {\n    Component.onCompleted: Qt.createQmlObject("x", this)\n}\n' 2
printf 'function f() { return Qt.createQmlObject("x", p); }\n' > "$tmp/shell/x.js"
if run >/dev/null; then echo "FAIL (accepted): createQmlObject in .js"; fail=1; fi
rm -f "$tmp/shell/x.js"

[ $fail = 0 ] && echo "lint_test ok"
exit $fail
