import QtQuick
import QtQuick.Layouts
import Quickshell
import "../core"
import "../logic/barlayout.js" as L
import "islands"

// The bar's own menu, on a right click on an island (or on the bar between islands): move the
// island left or right, change its size, take it off the bar, add another island next to it,
// or open the bar's page in Settings. Every edit goes through BarLayout (SetBarLayout), so it is
// saved and every screen's bar follows. Keyboard: Up/Down, Enter, Esc (IslandPopup). Never above the lock.
IslandPopup {
    id: pop

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

    onPageChanged: if (page === "main" && visible) focusFirst.restart()

    ColumnLayout {
        id: col
        width: Math.max(Theme.fontSize * 18, implicitWidth)
        spacing: 2

        UqText {
            text: pop.page === "add" ? "Add to the bar" : (pop.target ? L.name(pop.target.id) : "Bar")
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
                id: first
                visible: pop.target !== null && L.canMove(pop.bar, pop.target.zone, pop.target.index, -1)
                Layout.fillWidth: true
                text: "Move left"
                onActivated: { BarLayout.move(pop.target.zone, pop.target.index, -1); pop.hide(); }
            }
            MenuRow {
                visible: pop.target !== null && L.canMove(pop.bar, pop.target.zone, pop.target.index, 1)
                Layout.fillWidth: true
                text: "Move right"
                onActivated: { BarLayout.move(pop.target.zone, pop.target.index, 1); pop.hide(); }
            }
            Repeater {
                model: pop.target !== null ? L.SIZES : []
                delegate: MenuRow {
                    required property string modelData
                    Layout.fillWidth: true
                    text: modelData === "compact" ? "Compact" : (modelData === "wide" ? "Wide" : "Normal size")
                    detail: pop.target && pop.target.size === modelData ? "✓" : ""
                    Accessible.checkable: true
                    Accessible.checked: detail !== ""
                    onActivated: {
                        BarLayout.setSize(pop.target.zone, pop.target.index, modelData);
                        pop.hide();
                    }
                }
            }
            MenuRow {
                visible: pop.target !== null
                Layout.fillWidth: true
                text: "Remove from bar"
                onActivated: { BarLayout.remove(pop.target.zone, pop.target.index); pop.hide(); }
            }
            MenuRow {
                id: addEntry
                visible: L.addable(pop.bar).length > 0
                Layout.fillWidth: true
                text: "Add island…"
                onActivated: { pop.page = "add"; }
            }
            MenuRow {
                Layout.fillWidth: true
                text: "Edit the bar in Settings…"
                onActivated: {
                    Quickshell.execDetached(["uqctl", "settings", "shell", "islands"]);
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
                id: addList
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
        onTriggered: {
            var rows = [first, addEntry];
            for (var i = 0; i < rows.length; i++)
                if (rows[i].visible) {
                    rows[i].forceActiveFocus();
                    return;
                }
            col.forceActiveFocus();
        }
    }
}
