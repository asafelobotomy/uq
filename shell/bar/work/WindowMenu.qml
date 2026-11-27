// The window's menu: minimise, full or half width, float, pin, move, close, and the Island Menu.
import QtQuick
import QtQuick.Effects
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import "../../core"
import "../../services"
import "../../notif"
import "../islands"
import "../../logic/minimise.js" as Min
import "../../logic/work.js" as Work
import "../../logic/dock.js" as Dock

ColumnLayout {
    id: mcol
    // Passed in from WorkIsland.qml.
    property var menuPopup
    property var root

    spacing: 2
    width: Theme.fontSize * 20
    readonly property var w: root.menuWindow
    readonly property var tl: w ? root.toplevelFor(w.address) : null
    readonly property var ipc: tl && tl.lastIpcObject ? tl.lastIpcObject : ({})
    function done() { menuPopup.hide(); }

    UqText {
        text: mcol.w ? (mcol.w.title || mcol.w.cls) : ""
        baseBold: true
        Layout.fillWidth: true
    }
    MenuRow {
        id: firstRow
        Layout.fillWidth: true
        text: mcol.w && mcol.w.minimised ? "Restore" : "Minimise"
        onActivated: {
            if (mcol.w.minimised)
                root.choose(mcol.w);
            else
                root.act(mcol.w, "minimise");
            mcol.done();
        }
        Component.onCompleted: forceActiveFocus()
    }
    MenuRow {
        visible: mcol.w !== null && !mcol.w.minimised
        Layout.fillWidth: true
        text: mcol.w && root.isFull(mcol.w) ? "Half width" : "Full width"
        onActivated: { root.setFull(mcol.w, !root.isFull(mcol.w)); mcol.done(); }
    }
    MenuRow {
        visible: mcol.w !== null && !mcol.w.minimised
        Layout.fillWidth: true
        text: mcol.ipc["floating"] === true ? "Tile" : "Float"
        onActivated: { root.act(mcol.w, "toggle_float"); mcol.done(); }
    }
    MenuRow {
        visible: mcol.w !== null && Dock.canPin({ id: root.appOf(mcol.w.cls).id || "class:" })
            && root.appOf(mcol.w.cls).id !== ""
        Layout.fillWidth: true
        text: mcol.w && root.isPinned(mcol.w) ? "Unpin the app" : "Pin the app"
        onActivated: { root.togglePin(mcol.w); mcol.done(); }
    }
    UqText {
        visible: mcol.w !== null && !mcol.w.minimised
        text: "Move to workspace"
        color: Theme.textMuted
    }
    Flow {
        visible: mcol.w !== null && !mcol.w.minimised
        Layout.fillWidth: true
        spacing: 2
        Repeater {
            model: Work.wsChoices()
            delegate: MenuRow {
                required property int modelData
                text: String(modelData)
                Accessible.name: "Move to workspace " + modelData
                onActivated: { root.act(mcol.w, "move", { workspace: modelData }); mcol.done(); }
            }
        }
    }
    MenuRow {
        Layout.fillWidth: true
        text: "Close"
        detail: "Middle click"
        onActivated: { root.act(mcol.w, "close"); mcol.done(); }
    }
    MenuRow {
        Layout.fillWidth: true
        Layout.topMargin: Theme.gapIn
        text: "uQ Island Menu…"
        onActivated: {
            mcol.done();
            root.menuRequested(root);
        }
    }
}
