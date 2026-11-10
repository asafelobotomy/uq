// The only text item in the Settings app: a Controls Label that never interprets markup.
// scripts/lint-settings.sh forbids Text, Label, TextEdit and TextInput everywhere else.
import QtQuick
import QtQuick.Controls as Controls

Controls.Label {
    textFormat: Text.PlainText
    Accessible.name: text
}
