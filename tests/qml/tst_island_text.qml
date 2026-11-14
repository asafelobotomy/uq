import QtQuick
import QtTest
import "../../shell/core"

// UqText inside a bar island takes the island's text style (Island.qml's text* properties);
// text outside any island (a popup) keeps its own.
TestCase {
    name: "island_text"

    Item {
        id: island
        property bool isIsland: true
        property string textFamily: ""
        property real textScale: 1
        property bool textBold: false
        property bool textItalic: false
        property bool textCaps: false
        Item {
            UqText { id: plain; text: "Plain" }
            UqText { id: figures; text: "123"; baseFamily: "JetBrains Mono"; baseSize: 20; baseBold: true }
        }
    }
    UqText { id: outside; text: "Popup" }

    function cleanup() {
        island.textFamily = "";
        island.textScale = 1;
        island.textBold = false;
        island.textItalic = false;
        island.textCaps = false;
    }

    function test_theme_look_by_default() {
        compare(plain.styleHost, island);
        compare(outside.styleHost, null);
        compare(plain.font.family, Theme.fontUi);
        compare(plain.font.pixelSize, Theme.fontSize);
        compare(figures.font.family, "JetBrains Mono", "keeps its own font");
        verify(figures.font.bold);
    }

    function test_island_style_applies() {
        island.textFamily = "Serif";
        island.textScale = 1.5;
        island.textItalic = true;
        island.textCaps = true;
        island.textBold = true;
        compare(plain.font.family, "Serif");
        compare(figures.font.family, "Serif", "a chosen font wins over the island's own");
        compare(plain.font.pixelSize, Math.round(Theme.fontSize * 1.5));
        compare(figures.font.pixelSize, 30, "scales the island's own size");
        verify(plain.font.italic && plain.font.bold);
        compare(plain.font.capitalization, Font.AllUppercase);
        // Popup text is untouched.
        compare(outside.font.family, Theme.fontUi);
        compare(outside.font.pixelSize, Theme.fontSize);
        verify(!outside.font.italic && !outside.font.bold);
    }
}
