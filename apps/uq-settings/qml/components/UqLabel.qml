// The only text item in the Settings app: a Controls Label that never interprets markup.
// scripts/lint-settings.sh forbids Text, Label, TextEdit and TextInput everywhere else.
import QtQuick
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

Controls.Label {
    // On a highlighted (selected or pressed) row: a custom contentItem doesn't get the style's
    // switch to the highlighted text colour, so bind the row's state here. Text on the accent
    // needs the theme's accent text colour to stay readable.
    property bool highlighted: false

    textFormat: Text.PlainText
    color: highlighted ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor
    Accessible.name: text
}
