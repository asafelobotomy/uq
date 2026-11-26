// Text: font, formatting, size (the island's face; its popup keeps the theme's).
import QtQuick
import QtQuick.Layouts
import Quickshell
import "../../core"
import "../../logic/barlayout.js" as L
import "../../logic/ticker.js" as T
import "../../logic/weather.js" as Weather
import "../../logic/work.js" as Work
import "../islands"

ColumnLayout {
    // Passed in from IslandForm.qml.
    property var pop

    visible: pop.island !== L.SEPARATOR
    spacing: 2
    Layout.fillWidth: true
    Layout.topMargin: Theme.gapIn

    RowLayout {
        Layout.fillWidth: true
        UqText {
            text: "Text"
            color: Theme.textMuted
            Layout.fillWidth: true
        }
        MenuRow {
            visible: pop.ownText
            text: L.isPlainText(BarLayout.allText) ? "Use the theme's" : "Use the all-islands text"
            onActivated: pop.setText(null)
        }
    }
    UqText {
        text: "Font"
        color: Theme.textMuted
    }
    Flow {
        spacing: 2
        Layout.fillWidth: true
        MenuRow {
            text: "Theme"
            detail: pop.textStyle.font === null ? "✓" : ""
            Accessible.checkable: true
            Accessible.checked: pop.textStyle.font === null
            onActivated: pop.setText({ font: null })
        }
        MenuRow {
            text: "Monospace"
            detail: pop.textStyle.font === L.MONO ? "✓" : ""
            Accessible.checkable: true
            Accessible.checked: pop.textStyle.font === L.MONO
            onActivated: pop.setText({ font: L.MONO })
        }
        // A font picked from the search below.
        MenuRow {
            visible: pop.textStyle.font !== null && pop.textStyle.font !== L.MONO
            text: pop.textStyle.font || ""
            detail: "✓"
            Accessible.checkable: true
            Accessible.checked: true
        }
    }
    UqInput {
        id: fontInput
        Layout.fillWidth: true
        maximumLength: 64
        placeholder: "Find a font…"
        Accessible.name: "Find a font"
        onEdited: function (s) { pop.fontQuery = s; }
        Keys.onDownPressed: nextItemInFocusChain(true).forceActiveFocus()
    }
    Repeater {
        model: pop.fontMatches
        delegate: MenuRow {
            required property string modelData
            text: modelData
            detail: pop.textStyle.font === modelData ? "✓" : ""
            // Each name in its own font.
            textItem.baseFamily: modelData
            Layout.fillWidth: true
            onActivated: pop.setText({ font: modelData })
        }
    }
    UqText {
        visible: pop.fontQuery.trim() !== "" && pop.fontMatches.length === 0
        text: "No installed font matches"
        color: Theme.textMuted
    }

    UqText {
        text: "Formatting"
        color: Theme.textMuted
        Layout.topMargin: Theme.gapIn
    }
    Flow {
        spacing: 2
        Layout.fillWidth: true
        MenuRow {
            text: "Bold"
            textItem.baseBold: true
            detail: pop.textStyle.bold ? "✓" : ""
            Accessible.checkable: true
            Accessible.checked: pop.textStyle.bold
            onActivated: pop.setText({ bold: !pop.textStyle.bold })
        }
        MenuRow {
            text: "Italic"
            textItem.font.italic: true
            detail: pop.textStyle.italic ? "✓" : ""
            Accessible.checkable: true
            Accessible.checked: pop.textStyle.italic
            onActivated: pop.setText({ italic: !pop.textStyle.italic })
        }
        MenuRow {
            text: "All capitals"
            detail: pop.textStyle.caps ? "✓" : ""
            Accessible.checkable: true
            Accessible.checked: pop.textStyle.caps
            onActivated: pop.setText({ caps: !pop.textStyle.caps })
        }
    }

    UqText {
        text: "Text size: " + pop.textStyle.size + "%"
        color: Theme.textMuted
        Layout.topMargin: Theme.gapIn
    }
    RowLayout {
        spacing: 2
        MenuRow {
            text: "Smaller"
            detail: "−"
            enabled: L.stepTextSize(pop.textStyle.size, -1) !== null
            onActivated: {
                var n = L.stepTextSize(pop.textStyle.size, -1);
                if (n !== null)
                    pop.setText({ size: n });
            }
        }
        MenuRow {
            text: "Larger"
            detail: "+"
            enabled: L.stepTextSize(pop.textStyle.size, 1) !== null
            onActivated: {
                var n = L.stepTextSize(pop.textStyle.size, 1);
                if (n !== null)
                    pop.setText({ size: n });
            }
        }
    }
}
