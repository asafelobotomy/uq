#!/usr/bin/env bash
# lint-settings.sh must fail on Text/Label/TextEdit/TextInput (and createQmlObject) anywhere under
# apps/uq-settings/qml except components/UqLabel.qml, and pass on UqLabel.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
qml="$tmp/apps/uq-settings/qml"
mkdir -p "$tmp/scripts" "$qml/components" "$qml/pages"
cp "$root/scripts/lint-settings.sh" "$root/scripts/lib-qml-lint.sh" "$root/scripts/lint-control-text.awk" "$root/scripts/lint-jargon.awk" "$root/scripts/settings-jargon.txt" "$tmp/scripts/" || exit 1
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

# Stock controls: own `text:` is a literal only; data goes through a UqLabel contentItem.
bad control_bound_text Case.qml 'Item {\n  QQC2.CheckBox {\n    text: modelData.name\n  }\n}\n' 3
bad combo_without_contentitem Case.qml 'Item {\n  QQC2.ComboBox {\n    model: m\n    delegate: D {}\n  }\n}\n' 2
good control_literal 'Item {\n  QQC2.CheckBox { text: "Show seconds" }\n}\n'
good combo_full 'Item {\n  QQC2.ComboBox {\n    id: c\n    model: m\n    contentItem: UqLabel { text: c.displayText }\n    delegate: D {}\n  }\n}\n'
good nested_uqlabel_text 'Item {\n  QQC2.Button {\n    contentItem: UqLabel {\n      text: modelData\n    }\n  }\n}\n'
bad templates_prefix Case.qml 'Item {\n  T.CheckBox {\n    text: modelData\n  }\n}\n' 3
bad uqcombobox_text Case.qml 'Item {\n  UqComboBox {\n    text: modelData\n  }\n}\n' 3
good uqcombobox_plain 'Item {\n  UqComboBox { model: m }\n}\n'

# Jargon: a listed word inside a quoted string fails; the same line with `// jargon-ok:` passes.
bad jargon Case.qml 'Item {\n  UqLabel { text: "Restart uqd" }\n}\n' 2
bad jargon_case pages/P.qml 'Item {\n  UqLabel { text: "Open HyprCtl" }\n}\n' 2
bad jargon_dotted Case.qml 'Item {\n  UqLabel { text: "see state.toml now" }\n}\n' 2
bad jargon_hyphen Case.qml 'Item {\n  UqLabel { text: "the D-Bus name" }\n}\n' 2
bad jargon_single_quote Case.qml "Item {\\n  UqLabel { text: 'uqd' }\\n}\\n" 2
bad jargon_after_comment_text Case.qml 'Item {\n  UqLabel { text: "uqd" } // not an exemption\n}\n' 2
bad jargon_ok_no_reason Case.qml 'Item {\n  UqLabel { text: "uqd" } // jargon-ok:\n}\n' 2
bad jargon_ok_blank_reason Case.qml 'Item {\n  UqLabel { text: "uqd" } // jargon-ok:   \n}\n' 2
good jargon_ok 'Item {\n  UqLabel { text: "Restart uqd" } // jargon-ok: test\n}\n'
good jargon_in_comment 'Item {\n  // uqd is the service\n}\n'
good jargon_identifier 'Item {\n  objectName: "uqdDown"\n  visible: App.uqdUp\n}\n'
good jargon_allowed_words 'Item {\n  UqLabel { text: "Style Qt apps with Kvantum" }\n}\n'
good plain_words 'Item {\n  UqLabel { text: "Start the uQ desktop service" }\n}\n'
printf 'Item {\n  UqLabel { text: "uqd" }\n}\n' > "$qml/components/AdvancedDetails.qml"
if ! out=$(run); then echo "FAIL (AdvancedDetails.qml must be exempt): $out"; fail=1; fi
rm -f "$qml/components/AdvancedDetails.qml"

[ $fail = 0 ] && echo "lint_settings ok"
exit $fail
