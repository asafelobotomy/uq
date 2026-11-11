import QtQuick
import "../core"
import "../widgets"

// The right-click menu of an app row in Spotlight and the app menu: the app actions other apps
// installed for it (Phase 4 spec §2.4.2), names as plain text. Choosing one asks the bridge to
// run it (`Launch {app_action, desktop_id}`); the shell never builds a command line. Fills its
// parent while open, so a click outside closes it.
Item {
    id: menu

    property var registry: Widgets
    property string appId: ""
    property var actions: []
    property bool open: false

    // An action was started: the launcher closes.
    signal done()

    anchors.fill: parent
    visible: open
    z: 100

    // Open at (x, y) of `item` for the row of `appId`. False (and nothing shown) when no action
    // applies to it.
    function openAt(item, x, y, id) {
        var list = registry.actionsFor(id);
        if (list.length === 0 || Bridge.locked) {
            close();
            return false;
        }
        appId = id;
        actions = list;
        var p = item.mapToItem(menu, x, y);
        box.x = Math.max(0, Math.min(p.x, menu.width - box.width));
        box.y = Math.max(0, Math.min(p.y, menu.height - box.height));
        open = true;
        box.forceActiveFocus();
        return true;
    }

    function close() {
        open = false;
        actions = [];
        appId = "";
    }

    function choose(action) {
        var ok = registry.runAction(action, appId);
        close();
        if (ok)
            done();
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: menu.close()
    }

    Rectangle {
        id: box
        width: 220
        height: col.implicitHeight + Theme.gapIn * 2
        radius: Theme.radius - 2
        color: Theme.surface
        border.color: Theme.border
        border.width: Theme.borderWidth
        Accessible.role: Accessible.PopupMenu
        Accessible.name: "App actions"
        Keys.onEscapePressed: menu.close()

        Column {
            id: col
            x: Theme.gapIn
            y: Theme.gapIn
            width: box.width - Theme.gapIn * 2

            Repeater {
                model: menu.actions
                delegate: Rectangle {
                    id: entry
                    required property var modelData
                    width: col.width
                    height: 28
                    radius: Theme.radius - 2
                    color: entryMouse.containsMouse ? Theme.overlay : "transparent"
                    Accessible.role: Accessible.MenuItem
                    Accessible.name: String(modelData.name)
                    Accessible.onPressAction: menu.choose(entry.modelData)

                    UqText {
                        anchors.fill: parent
                        anchors.leftMargin: Theme.gapIn * 2
                        anchors.rightMargin: Theme.gapIn
                        verticalAlignment: Text.AlignVCenter
                        text: String(entry.modelData.name)
                    }
                    MouseArea {
                        id: entryMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: menu.choose(entry.modelData)
                    }
                }
            }
        }
    }
}
