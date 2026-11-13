import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import "../core"
import "../logic/barlayout.js" as L
import "islands"

// Edit mode's panel, at the bottom of the screen editing started on: what can be done, how many
// islands don't fit this screen, Add an Island, Reset and Done. It holds the keyboard while no
// island popup is open: Left/Right choose an island, Shift+Left/Right move it, + and - change
// its size, Enter opens its settings, Delete removes it, A adds one, Esc finishes.
PanelWindow {
    id: panel

    // The Bar this panel belongs to.
    property var bar: null
    property bool active: false

    screen: bar ? bar.screen : null
    visible: active && !Bridge.locked
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.namespace: "uq-bar-edit"
    WlrLayershell.keyboardFocus: BarLayout.activePopup ? WlrKeyboardFocus.None : WlrKeyboardFocus.Exclusive
    exclusionMode: ExclusionMode.Ignore
    anchors.bottom: true
    margins.bottom: Theme.gapOut * 2
    implicitWidth: box.implicitWidth
    implicitHeight: box.implicitHeight

    readonly property var order: bar ? L.order(BarLayout.layout, bar.fitted) : []
    readonly property int hiddenCount: bar ? bar.fitted.hiddenCount : 0

    function selectedAt() {
        for (var i = 0; i < order.length; i++)
            if (order[i].key === BarLayout.selected)
                return i;
        return -1;
    }
    function select(dir) {
        if (order.length === 0)
            return;
        var i = selectedAt();
        i = i < 0 ? (dir > 0 ? 0 : order.length - 1) : Math.max(0, Math.min(order.length - 1, i + dir));
        BarLayout.selected = order[i].key;
    }
    function current() {
        var i = selectedAt();
        return i < 0 ? null : order[i];
    }
    function resize(dir) {
        var c = current();
        if (c === null)
            return;
        var it = BarLayout.layout[c.zone][c.index];
        if (it.id === L.SEPARATOR)
            return;
        var s = L.stepSize(L.sizeOf(it), dir);
        if (s !== null)
            BarLayout.setSize(c.zone, c.index, s);
    }
    function addIsland() {
        var c = current();
        bar.openAdd(addButton, c);
    }

    Rectangle {
        id: box
        anchors.fill: parent
        implicitWidth: col.implicitWidth + Theme.gapOut * 2
        implicitHeight: col.implicitHeight + Theme.gapOut * 2
        color: Theme.surface
        radius: Theme.radius
        border.color: Theme.accent
        border.width: Theme.borderWidth
        focus: true
        Accessible.role: Accessible.ToolBar
        Accessible.name: "Editing the bar"

        Keys.onPressed: function (e) {
            var shift = (e.modifiers & Qt.ShiftModifier) !== 0;
            var c = panel.current();
            switch (e.key) {
            case Qt.Key_Escape:
                BarLayout.stopEdit();
                break;
            case Qt.Key_Left:
            case Qt.Key_Right:
                var dir = e.key === Qt.Key_Left ? -1 : 1;
                if (shift && c !== null)
                    BarLayout.move(c.zone, c.index, dir);
                else
                    panel.select(dir);
                break;
            case Qt.Key_Plus:
            case Qt.Key_Equal:
                panel.resize(1);
                break;
            case Qt.Key_Minus:
                panel.resize(-1);
                break;
            case Qt.Key_Return:
            case Qt.Key_Enter:
                if (c !== null)
                    panel.bar.openSettings(c.key);
                else
                    return;
                break;
            case Qt.Key_Delete:
            case Qt.Key_Backspace:
                if (c !== null)
                    BarLayout.remove(c.zone, c.index);
                break;
            case Qt.Key_A:
                panel.addIsland();
                break;
            default:
                return;
            }
            e.accepted = true;
        }

        ColumnLayout {
            id: col
            x: Theme.gapOut
            y: Theme.gapOut
            spacing: Theme.gapIn

            RowLayout {
                spacing: Theme.gapOut
                UqText {
                    text: "Editing the bar"
                    font.bold: true
                }
                UqText {
                    text: "Drag an island to move it, drag its edge to resize it, click it for its settings."
                    color: Theme.textMuted
                }
            }
            UqText {
                text: "← → choose   Shift+← → move   + − size   Enter settings   Delete remove   A add   Esc done"
                color: Theme.textMuted
            }
            UqText {
                visible: panel.hiddenCount > 0
                text: panel.hiddenCount === 1 ? "1 island doesn't fit on this screen. Make others smaller or remove some."
                    : panel.hiddenCount + " islands don't fit on this screen. Make others smaller or remove some."
                color: Theme.warning
            }
            UqText {
                visible: BarLayout.error !== ""
                text: BarLayout.error
                color: Theme.error
            }
            RowLayout {
                spacing: Theme.gapIn
                MenuRow {
                    id: addButton
                    text: "Add an Island…"
                    activeFocusOnTab: false
                    onActivated: panel.addIsland()
                }
                MenuRow {
                    text: "Reset the bar"
                    activeFocusOnTab: false
                    onActivated: BarLayout.reset()
                }
                Item {
                    Layout.fillWidth: true
                }
                MenuRow {
                    text: "Done"
                    detail: "Esc"
                    activeFocusOnTab: false
                    onActivated: BarLayout.stopEdit()
                }
            }
        }
    }

    onVisibleChanged: if (visible) box.forceActiveFocus()
}
