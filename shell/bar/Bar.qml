import QtQuick
import Quickshell
import Quickshell.Wayland
import "../core"
import "../logic/barlayout.js" as L
import "islands"

// One bar per screen, laid out from `shell.bar` (BarLayout): three zones of islands, left, centre
// (truly centred) and right, each island Icon, Small, Medium or Large: 1, 2, 4 or 6 times the
// Icon size (a square island as tall as the bar). Islands never overlap and are never cut off:
// when they don't all fit, the biggest step down a size, and only when every island is at Icon
// size are the most expendable left out (barlayout.js fit). A right click on an island (or the
// bar) opens the island menu (BarMenu.qml): edit the islands, add one, remove this one. In edit
// mode (BarLayout.editing) islands are dragged to move, resized by their edge grip and set up
// with a left click; their own clicks are off until editing finishes (Esc, or Done).
PanelWindow {
    id: bar
    required property ShellScreen modelData
    screen: modelData

    readonly property int islandHeight: BarLayout.unit
    readonly property var layout: BarLayout.layout
    readonly property var metrics: BarLayout.metrics
    readonly property bool editing: BarLayout.editing
    // This bar holds the keyboard in edit mode (the screen editing started on).
    readonly property bool editsHere: editing && (BarLayout.editScreen === "" || BarLayout.editScreen === modelData.name)

    // The zones as drawn. Replaced only when an island is added, removed, moved or resized, not
    // when an island's options change, so islands (and their open popups) stay put meanwhile.
    property var zones: ({ start: [], center: [], end: [] })
    readonly property string zonesKey: JSON.stringify([layout.start, layout.center, layout.end])
    onZonesKeyChanged: zones = { start: layout.start, center: layout.center, end: layout.end }
    Component.onCompleted: zones = { start: layout.start, center: layout.center, end: layout.end }

    readonly property var fitted: L.fit(zones, width, metrics)
    readonly property var placedMap: L.place(zones, fitted, width, metrics)
    // Every island in bar order (hidden ones too, so delegates don't come and go with the width).
    readonly property var items: {
        var out = [];
        for (var z = 0; z < L.ZONES.length; z++) {
            var list = zones[L.ZONES[z]];
            for (var i = 0; i < list.length; i++)
                out.push({ zone: L.ZONES[z], index: i, id: list[i].id, key: L.key(L.ZONES[z], i) });
        }
        return out;
    }

    WlrLayershell.namespace: "uq-bar"
    anchors { top: true; left: true; right: true }
    margins { top: Theme.gapOut; left: Theme.gapOut; right: Theme.gapOut }
    implicitHeight: islandHeight
    exclusiveZone: islandHeight + Theme.gapOut
    color: "transparent"

    // Stay awake (quick settings): blocks every idle stage while on. One per bar; any one held
    // inhibits idle for the session.
    IdleInhibitor {
        window: bar
        enabled: StayAwake.enabled
    }

    function component(id) {
        switch (id) {
        case "system": return cSystem;
        case "work": return cWork;
        case "clock": return cClock;
        case "weather": return cWeather;
        case "notif": return cNotif;
        case "status": return cStatus;
        case "media": return cMedia;
        case "stocks": return cStocks;
        case "news": return cNews;
        case "sysmon": return cSysMon;
        case "worldclock": return cWorldClock;
        case "keyboard": return cKeyboard;
        case "separator": return cSeparator;
        }
        return null;
    }

    Component { id: cSystem; SystemIsland { screen: bar.modelData } }
    Component { id: cWork; WorkIsland { screen: bar.modelData } }
    Component { id: cClock; ClockIsland {} }
    Component { id: cWeather; WeatherIsland {} }
    Component { id: cNotif; NotifIsland { screen: bar.modelData } }
    Component { id: cStatus; StatusIsland { screen: bar.modelData } }
    Component { id: cMedia; MediaIsland {} }
    Component { id: cStocks; StocksIsland {} }
    Component { id: cNews; NewsIsland {} }
    Component { id: cSysMon; SysMonIsland {} }
    Component { id: cWorldClock; WorldClockIsland {} }
    Component { id: cKeyboard; KeyboardIsland {} }
    Component { id: cSeparator; SeparatorIsland {} }

    // The slot of island `key`, for anchoring popups to it.
    function slotFor(k) {
        for (var i = 0; i < slots.count; i++) {
            var s = slots.itemAt(i);
            if (s && s.key === k)
                return s;
        }
        return null;
    }
    function openSettings(k) {
        var s = slotFor(k);
        if (s === null || !s.visible)
            return;
        BarLayout.selected = k;
        islandSettings.showFor(s, { zone: s.zone, index: s.index, id: s.islandId, size: s.size });
    }

    // Edit mode on, with island `k`'s settings open.
    function editIsland(k) {
        BarLayout.selected = k;
        if (BarLayout.editing) {
            openSettings(k);
            return;
        }
        BarLayout.startEdit(modelData.name);
        BarLayout.selected = k;
        // After the edit panel has mapped and taken the keyboard, or its arrival would close the
        // settings again (the popup closes when focus leaves it).
        pendingSettings = k;
        settingsLater.restart();
    }
    property string pendingSettings: ""
    Timer {
        id: settingsLater
        interval: 250
        onTriggered: {
            var k = bar.pendingSettings;
            bar.pendingSettings = "";
            if (k !== "" && BarLayout.editing)
                bar.openSettings(k);
        }
    }

    // The island menu's "Add an Island" list, next to island `c` ({zone, index, key}) or, with
    // none, from the middle of the bar.
    function openAdd(_from, c) {
        var s = c ? slotFor(c.key) : null;
        if (s === null) {
            menuAnchor.x = width / 2;
            s = menuAnchor;
        }
        menu.showFor(s, c ? { zone: c.zone, index: c.index, id: zones[c.zone][c.index].id, size: s.size } : null);
        menu.page = "add";
    }

    // --- dragging in edit mode ---
    property string dragKey: ""
    property real dragX: 0
    readonly property var dropPoints: dragKey !== "" ? L.dropPoints(zones, placedMap, width) : []
    readonly property var dropAt: dragKey !== "" ? L.nearestDrop(dropPoints, dragX) : null
    property string resizeKey: ""
    property real resizeW: 0

    // The bar between islands: the island menu without "Remove".
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.RightButton
        onClicked: function (m) {
            menuAnchor.x = m.x;
            menu.showFor(menuAnchor, null);
        }
    }
    Item {
        id: menuAnchor
        width: 1
        height: bar.islandHeight
    }

    Repeater {
        id: slots
        model: bar.items
        delegate: Item {
            id: slot
            required property var modelData
            readonly property string key: modelData.key
            readonly property string zone: modelData.zone
            readonly property int index: modelData.index
            readonly property string islandId: modelData.id
            readonly property var place: bar.placedMap[key] || null
            readonly property string size: place ? place.size : L.sizeOf(bar.zones[zone][index])
            readonly property Item island: loader.item
            readonly property bool selected: bar.editing && BarLayout.selected === key
            // An island may hide itself (media with no player) outside edit mode.
            visible: place !== null && (bar.editing || island === null || island.shown !== false)
            x: place ? place.x : 0
            width: place ? place.w : 0
            height: bar.islandHeight

            // Outside edit mode: the island menu on a right click (an island's own right-click
            // handler, where it has one, is in front and wins).
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.RightButton
                onClicked: menu.showFor(slot, { zone: slot.zone, index: slot.index, id: slot.islandId, size: slot.size })
            }
            Loader {
                id: loader
                anchors.fill: parent
                sourceComponent: bar.component(slot.islandId)
                onLoaded: {
                    item.size = Qt.binding(function () { return slot.size; });
                    if (item.editRequested)
                        item.editRequested.connect(function () { bar.editIsland(slot.key); });
                    if (item.menuRequested)
                        item.menuRequested.connect(function (a) {
                            menu.showFor(a || slot, { zone: slot.zone, index: slot.index, id: slot.islandId, size: slot.size });
                        });
                }
                opacity: bar.dragKey === slot.key ? 0.4 : 1
            }

            // Edit mode: on top of the island, so its own clicks are off.
            Item {
                anchors.fill: parent
                visible: bar.editing
                z: 10

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.radius
                    color: "transparent"
                    border.color: Theme.accent
                    border.width: slot.selected ? Theme.borderWidth * 2 : Theme.borderWidth
                }
                MouseArea {
                    id: editMouse
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    cursorShape: bar.dragKey === slot.key ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                    property real pressX: 0
                    onPressed: function (m) { pressX = m.x; }
                    onPositionChanged: function (m) {
                        if (!(m.buttons & Qt.LeftButton))
                            return;
                        if (bar.dragKey === "" && Math.abs(m.x - pressX) > Theme.gapIn * 2)
                            bar.dragKey = slot.key;
                        if (bar.dragKey === slot.key)
                            bar.dragX = slot.x + m.x;
                    }
                    onReleased: function (m) {
                        if (bar.dragKey === slot.key) {
                            var pt = bar.dropAt;
                            bar.dragKey = "";
                            BarLayout.drop(slot.zone, slot.index, pt);
                        }
                    }
                    onClicked: function (m) {
                        if (m.button === Qt.RightButton)
                            menu.showFor(slot, { zone: slot.zone, index: slot.index, id: slot.islandId, size: slot.size });
                        else
                            bar.openSettings(slot.key);
                    }
                }
                // The resize grip on the right edge (not on separators): drag to a size.
                Rectangle {
                    visible: slot.islandId !== L.SEPARATOR
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    width: Theme.gapIn + Theme.borderWidth
                    height: parent.height * 0.6
                    radius: width / 2
                    color: Theme.accent
                    MouseArea {
                        anchors.fill: parent
                        anchors.margins: -Theme.gapIn
                        cursorShape: Qt.SizeHorCursor
                        property real startX: 0
                        onPressed: function (m) {
                            startX = mapToItem(null, m.x, 0).x;
                            bar.resizeW = slot.width;
                            bar.resizeKey = slot.key;
                        }
                        onPositionChanged: function (m) {
                            var dx = mapToItem(null, m.x, 0).x - startX;
                            bar.resizeW = Math.max(BarLayout.unit, Math.min(BarLayout.unit * 6, slot.width + dx));
                        }
                        onReleased: {
                            var s = L.sizeForWidth(bar.resizeW, BarLayout.unit);
                            bar.resizeKey = "";
                            BarLayout.selected = slot.key;
                            if (s !== L.sizeOf(bar.zones[slot.zone][slot.index]))
                                BarLayout.setSize(slot.zone, slot.index, s);
                        }
                    }
                }
            }

            // Resizing: the size it will snap to, drawn over the bar.
            Rectangle {
                visible: bar.resizeKey === slot.key
                z: 20
                x: 0
                width: L.UNITS[L.sizeForWidth(bar.resizeW, BarLayout.unit)] * BarLayout.unit
                height: parent.height
                radius: Theme.radius
                color: "transparent"
                border.color: Theme.accent
                border.width: Theme.borderWidth * 2
                Rectangle {
                    anchors.centerIn: parent
                    width: sizeLabel.implicitWidth + Theme.gapIn * 2
                    height: sizeLabel.implicitHeight + Theme.gapIn
                    radius: Theme.radius
                    color: Theme.accent
                    UqText {
                        id: sizeLabel
                        anchors.centerIn: parent
                        text: L.SIZE_NAMES[L.sizeForWidth(bar.resizeW, BarLayout.unit)]
                        color: Theme.accentText
                    }
                }
            }
        }
    }

    // Moving: where the island will land.
    Rectangle {
        visible: bar.dropAt !== null
        x: bar.dropAt ? Math.max(0, Math.min(bar.width - width, bar.dropAt.x - width / 2)) : 0
        width: Theme.borderWidth * 2
        height: bar.islandHeight
        radius: width / 2
        color: Theme.accent
        z: 30
    }

    BarMenu {
        id: menu
        barScreen: bar.modelData
    }

    IslandSettings {
        id: islandSettings
    }

    // The edit-mode panel under the bar, on the screen editing started on.
    EditPanel {
        bar: bar
        active: bar.editsHere
    }
}
