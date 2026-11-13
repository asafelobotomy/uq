import QtQuick

// The only Text in the shell: all strings are untrusted, so they are never parsed as rich text.
// scripts/lint-shell.sh forbids a bare Text outside this file.
//
// Set the font through baseFamily, baseSize and baseBold, not font.*: on a bar island, the
// island's own text style (edit mode: font, bold, italic, capitals, size) is laid over them.
// The island is the nearest ancestor with `isIsland` (Island.qml); a popup's text has none.
Text {
    property string baseFamily: Theme.fontUi
    property real baseSize: Theme.fontSize
    property bool baseBold: false
    readonly property Item styleHost: {
        for (var p = parent; p; p = p.parent)
            if (p.isIsland === true)
                return p;
        return null;
    }

    textFormat: Text.PlainText
    color: Theme.text
    font.family: styleHost !== null && styleHost.textFamily !== "" ? styleHost.textFamily : baseFamily
    font.pixelSize: Math.max(1, Math.round(baseSize * (styleHost !== null ? styleHost.textScale : 1)))
    font.bold: baseBold || (styleHost !== null && styleHost.textBold)
    font.italic: styleHost !== null && styleHost.textItalic
    font.capitalization: styleHost !== null && styleHost.textCaps ? Font.AllUppercase : Font.MixedCase
    elide: Text.ElideRight
}
