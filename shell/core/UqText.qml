import QtQuick

// The only Text in the shell: all strings are untrusted, so they are never parsed as rich text.
// scripts/lint-shell.sh forbids a bare Text outside this file.
Text {
    textFormat: Text.PlainText
    color: Theme.text
    font.family: Theme.fontUi
    font.pixelSize: Theme.fontSize
    elide: Text.ElideRight
}
