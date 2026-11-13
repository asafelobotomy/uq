import QtQuick
import Quickshell
import Quickshell.Wayland
import "../core"
import "islands"

// One bar per screen, laid out from `shell.bar` (BarLayout): three zones of floating islands,
// left, centre (truly centred) and right. Each island is loaded only while it is placed. A right
// click on an island without its own right-click action, or on the bar between islands, opens
// the bar menu (BarMenu.qml) to move, resize, remove or add islands.
PanelWindow {
    id: bar
    required property ShellScreen modelData
    screen: modelData

    readonly property int islandHeight: 36
    readonly property var layout: BarLayout.layout
    // The zones as drawn. Replaced only when an island is added, removed, moved or resized, not
    // when an island's options change, so islands (and their open popups) stay put meanwhile.
    property var zones: ({ start: [], center: [], end: [] })
    readonly property string zonesKey: JSON.stringify([layout.start, layout.center, layout.end])
    onZonesKeyChanged: zones = { start: layout.start, center: layout.center, end: layout.end }
    Component.onCompleted: zones = { start: layout.start, center: layout.center, end: layout.end }
    readonly property real gap: Theme.gapIn

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

    // When the islands don't all fit, the right zone keeps its place, the centre zone moves left
    // of centre rather than cover it, and the left zone gets what is left: the Work island fits
    // its titles to that, and anything still too wide is cut off rather than drawn underneath.
    readonly property real endLeft: zones.end.length > 0 ? width - endRow.width - gap : width
    readonly property real centerX: Math.max(0, Math.min((width - centerRow.width) / 2,
                                                         endLeft - centerRow.width))
    // Room for the left zone (the Work island sizes its titles from it).
    readonly property real startRoom: Math.max(0, (zones.center.length > 0 ? centerX : endLeft) - gap)

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
    Component { id: cWork; WorkIsland { screen: bar.modelData; barWidth: bar.startRoom } }
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

    // An island in a zone: the loaded island, and behind it the bar menu's right click (an
    // island's own right-click handler, where it has one, is in front and wins).
    component Slot: Item {
        id: slot
        required property var modelData
        required property int index
        property string zone: ""
        readonly property var island: loader.item
        visible: island !== null && island.shown !== false
        width: island ? island.implicitWidth : 0
        height: bar.islandHeight

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.RightButton
            onClicked: menu.showFor(slot, {
                zone: slot.zone, index: slot.index, id: slot.modelData.id, size: slot.modelData.size
            })
        }
        Loader {
            id: loader
            anchors.fill: parent
            sourceComponent: bar.component(slot.modelData.id)
            onLoaded: item.size = Qt.binding(function () { return slot.modelData.size; })
        }
    }

    // The bar between islands: the menu with only "Add island…" and Settings.
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

    Item {
        anchors.left: parent.left
        width: Math.min(startRow.width, bar.startRoom)
        height: parent.height
        clip: startRow.width > width
        Row {
            id: startRow
            height: parent.height
            spacing: bar.gap
            Repeater {
                model: bar.zones.start
                delegate: Slot { zone: "start" }
            }
        }
    }
    Item {
        x: bar.centerX
        width: Math.min(centerRow.width, Math.max(0, bar.endLeft - bar.centerX))
        height: parent.height
        clip: centerRow.width > width
    Row {
        id: centerRow
        height: parent.height
        spacing: bar.gap
        Repeater {
            model: bar.zones.center
            delegate: Slot { zone: "center" }
        }
    }
    }
    Row {
        id: endRow
        anchors.right: parent.right
        height: parent.height
        spacing: bar.gap
        Repeater {
            model: bar.zones.end
            delegate: Slot { zone: "end" }
        }
    }

    BarMenu {
        id: menu
    }
}
