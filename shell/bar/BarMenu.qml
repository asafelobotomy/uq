import QtQuick
import QtQuick.Layouts
import Quickshell
import "../core"
import "../logic/barlayout.js" as L
import "islands"

// The uQ Island Menu, on a right click on an island (or on the bar between islands): Edit
// Islands (edit mode, where islands are moved, resized and set up), Add an Island (next to this
// one), and Remove (this one). Every change goes through BarLayout (SetBarLayout), so it is saved
// and every screen's bar follows. Keyboard: Up/Down, Enter, Esc (IslandPopup).
IslandPopup {
    id: pop

    // The bar's screen, where edit mode takes the keyboard.
    property var barScreen: null
    // The island the menu is for: {zone, index, id, size}; null on the bar's empty space.
    property var target: null
    // "main" or "add"
    property string page: "main"

    function showFor(item, t) {
        target = t;
        page = "main";
        show(item);
        focusFirst.restart();
    }

    readonly property var bar: BarLayout.layout

    onPageChanged: if (visible) focusFirst.restart()

    ColumnLayout {
        id: col
        width: Math.max(Theme.fontSize * 18, implicitWidth)
        spacing: 2
        Accessible.role: Accessible.PopupMenu
        Accessible.name: "uQ Island Menu"

        UqText {
            text: "uQ Island Menu"
            baseBold: true
            Layout.fillWidth: true
        }
        UqText {
            text: pop.page === "add" ? "Add an island" : (pop.target ? L.name(pop.target.id) : "The bar")
            color: Theme.textMuted
            Layout.fillWidth: true
            Layout.bottomMargin: Theme.gapIn
        }

        // --- main ---
        ColumnLayout {
            visible: pop.page === "main"
            spacing: 2
            Layout.fillWidth: true

            MenuRow {
                id: editRow
                Layout.fillWidth: true
                text: BarLayout.editing ? "Finish editing" : "Edit Islands"
                detail: BarLayout.editing ? "Esc" : ""
                onActivated: {
                    pop.hide();
                    BarLayout.toggleEdit(pop.barScreen ? pop.barScreen.name : "");
                }
            }
            MenuRow {
                id: addEntry
                visible: L.addable(pop.bar).length > 0
                Layout.fillWidth: true
                text: "Add an Island…"
                onActivated: pop.page = "add"
            }
            MenuRow {
                visible: pop.target !== null
                Layout.fillWidth: true
                text: "Remove"
                onActivated: {
                    BarLayout.remove(pop.target.zone, pop.target.index);
                    pop.hide();
                }
            }
            UqText {
                visible: BarLayout.error !== ""
                text: BarLayout.error
                color: Theme.error
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
        }

        // --- add ---
        ColumnLayout {
            visible: pop.page === "add"
            spacing: 2
            Layout.fillWidth: true
            Repeater {
                model: pop.page === "add" ? L.addable(pop.bar) : []
                delegate: MenuRow {
                    required property var modelData
                    required property int index
                    Layout.fillWidth: true
                    text: modelData.name
                    onActivated: {
                        var t = pop.target;
                        if (t !== null)
                            BarLayout.add(modelData.id, t.zone, t.index + 1);
                        else
                            BarLayout.add(modelData.id, "end", 0);
                        pop.hide();
                    }
                    Component.onCompleted: if (index === 0) forceActiveFocus()
                }
            }
            MenuRow {
                Layout.fillWidth: true
                text: "Back"
                onActivated: pop.page = "main"
            }
        }
    }

    Timer {
        id: focusFirst
        interval: 0
        onTriggered: if (pop.page === "main") editRow.forceActiveFocus()
    }
}
