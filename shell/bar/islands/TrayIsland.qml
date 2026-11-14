import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Services.SystemTray
import Quickshell.Widgets
import "../../core"
import "../../notif"
import "../../logic/net.js" as Net
import "../../logic/tray.js" as Tray
import ".."

// Other apps' tray icons (`tray`), apart from uQ's own Status island: as many as fit the
// island's width, and when they don't all fit, a "+N" button listing the rest. Icons uQ shows
// elsewhere stay out (logic/tray.js: nm-applet beside uQ's network icon, noti-fi's icon while it
// is the bell). While this island is on the bar, Status shows no tray icons. Hidden outside
// edit mode while no app has an icon in the tray.
Island {
    id: root
    readonly property int iconSize: 18
    readonly property int cell: 26

    readonly property var items: {
        var all = SystemTray.items.values;
        var bell = Tray.notifiBell(NotifBackend.backend, Tray.find(all, Tray.NOTIFI));
        return Tray.shownItems(all, { notifiBell: bell });
    }
    readonly property var fit: Tray.fit(items.length, room, cell, Theme.gapIn)
    readonly property var shownItems: items.slice(0, fit.shown)
    readonly property var moreItems: items.slice(fit.shown)

    shown: items.length > 0
    Accessible.role: Accessible.ToolBar
    Accessible.name: items.length === 1 ? "1 tray icon" : items.length + " tray icons"

    RowLayout {
        anchors.centerIn: parent
        spacing: Theme.gapIn

        Repeater {
            model: root.shownItems
            delegate: TrayIcon {
                cell: root.cell
                iconSize: root.iconSize
                Layout.preferredWidth: root.cell
                Layout.preferredHeight: root.cell
            }
        }
        // The rest, in a list.
        Rectangle {
            id: moreButton
            visible: root.fit.more > 0
            Layout.preferredWidth: root.cell
            Layout.preferredHeight: root.cell
            radius: Theme.radius
            color: moreMouse.containsMouse || morePopup.open ? Theme.surfaceAlt : "transparent"
            Accessible.role: Accessible.Button
            Accessible.name: root.fit.more + " more tray icons"
            Accessible.onPressAction: root.showMore()
            UqText {
                anchors.centerIn: parent
                text: "+" + root.fit.more
                color: Theme.textMuted
            }
            MouseArea {
                id: moreMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: root.showMore()
            }
        }
    }

    function showMore() {
        if (morePopup.open)
            morePopup.hide();
        else
            morePopup.show(moreButton, moreBody);
    }

    LazyPopup {
        id: morePopup
    }
    Component {
        id: moreBody
        ColumnLayout {
            spacing: 2
            width: Theme.fontSize * 22
            Repeater {
                model: root.moreItems
                delegate: MenuRow {
                    id: moreRow
                    required property var modelData
                    required property int index
                    Layout.fillWidth: true
                    text: modelData.tooltipTitle || modelData.title || modelData.id || "Tray icon"
                    detail: modelData.hasMenu ? "Menu" : ""
                    onActivated: {
                        morePopup.hide();
                        // An app with only a menu opens it, under the island.
                        if (modelData.onlyMenu && modelData.hasMenu) {
                            moreMenu.menu = modelData.menu;
                            moreMenu.open();
                        } else {
                            modelData.activate();
                        }
                    }
                    Component.onCompleted: if (index === 0) forceActiveFocus()
                }
            }
        }
    }
    QsMenuAnchor {
        id: moreMenu
        anchor.item: root
        anchor.edges: Edges.Bottom
        anchor.gravity: Edges.Bottom
    }
}
