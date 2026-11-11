import QtQuick
import "../core"
import "../logic/widgets.js" as W

// The slots of one kind (Phase 4 spec §2.2). "bar": up to 4 one-cell icons in a row, the rest
// listed under a "…" button (overflowClicked; the bar shows them in a popup). "quick": one row
// per tile, full width, for the Apps group of quick settings. Delegates are kept across list
// updates, so a widget isn't reloaded when another one comes or goes.
Item {
    id: host

    property string kind: "bar"
    property var registry: Widgets
    property var bridge: Bridge
    property int max: W.BAR_MAX
    property int cell: 26
    property int spacing: Theme.gapIn
    // Bar: the widgets to list under "…"; set from outside to host them elsewhere.
    property var widgets: kind === "bar" ? W.barLayout(registry.list, max).shown
                                         : W.inSlot(registry.list, "quick")
    readonly property var overflow: kind === "bar" ? W.barLayout(registry.list, max).overflow : []
    property int loadedCount: 0
    property bool showMore: kind === "bar"
    readonly property bool moreShown: showMore && overflow.length > 0

    signal overflowClicked(Item anchor)
    // A slot asked to open or close its widget's popup (or detail view).
    signal popupRequested(Item slot, bool open)

    implicitWidth: kind === "bar" ? grid.implicitWidth : 200
    implicitHeight: grid.implicitHeight

    // The slot at index i (tests, popups), or null.
    function slotAt(i) {
        return rep.itemAt(i);
    }

    function recount() {
        var n = 0;
        for (var i = 0; i < rep.count; i++) {
            var it = rep.itemAt(i);
            if (it && it.loaded)
                n++;
        }
        loadedCount = n;
    }

    // Bring `entries` in with as few model changes as possible.
    function sync(list) {
        var want = list || [];
        var ids = want.map(function (w) { return w.id; });
        for (var r = entries.count - 1; r >= 0; r--)
            if (ids.indexOf(entries.get(r).wid) < 0)
                entries.remove(r);
        for (var i = 0; i < want.length; i++) {
            var w = want[i];
            var at = -1;
            for (var j = i; j < entries.count; j++)
                if (entries.get(j).wid === w.id) {
                    at = j;
                    break;
                }
            if (at < 0) {
                entries.insert(i, { wid: w.id, name: w.name, qmlUrl: w.qmlUrl });
                continue;
            }
            if (at !== i)
                entries.move(at, i, 1);
            if (entries.get(i).name !== w.name)
                entries.setProperty(i, "name", w.name);
            if (entries.get(i).qmlUrl !== w.qmlUrl)
                entries.setProperty(i, "qmlUrl", w.qmlUrl);
        }
        Qt.callLater(recount);
    }

    onWidgetsChanged: sync(widgets)
    Component.onCompleted: sync(widgets)

    ListModel {
        id: entries
    }

    Grid {
        id: grid
        width: host.kind === "bar" ? implicitWidth : host.width
        rows: host.kind === "bar" ? 1 : 0
        columns: host.kind === "bar" ? 0 : 1
        spacing: host.spacing
        verticalItemAlignment: Grid.AlignVCenter

        Repeater {
            id: rep
            model: entries
            delegate: Slot {
                id: s
                required property string wid
                required property string name
                required property string qmlUrl
                kind: host.kind
                cell: host.cell
                registry: host.registry
                bridge: host.bridge
                width: host.kind === "bar" ? host.cell : grid.width
                widget: ({ id: wid, name: name, qmlUrl: qmlUrl })
                onLoadedChanged: host.recount()
                onPopupRequested: function (open) { host.popupRequested(s, open); }
            }
            onItemAdded: host.recount()
            onItemRemoved: Qt.callLater(host.recount)
        }

        // "…": the widgets that don't fit.
        Rectangle {
            id: more
            objectName: "more"
            visible: host.moreShown
            width: host.cell
            height: host.cell
            radius: Theme.radius - 2
            color: moreMouse.containsMouse ? Theme.overlay : "transparent"
            Accessible.role: Accessible.Button
            Accessible.name: "More widgets"
            Accessible.onPressAction: host.overflowClicked(more)
            UqText {
                anchors.centerIn: parent
                text: "…"
                color: Theme.textMuted
            }
            MouseArea {
                id: moreMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: host.overflowClicked(more)
            }
        }
    }
}
