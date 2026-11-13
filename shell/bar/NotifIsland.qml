import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Services.SystemTray
import Quickshell.Widgets
import "../core"
import "../notif"
import "../logic/notif.js" as N
import "../logic/launcher.js" as L
import "../logic/net.js" as Net
import "../logic/tray.js" as Tray

// While noti-fi is the backend, its own tray icon stands in for the bell (and leaves the tray):
// left click does what it does in the tray, right click opens its menu, middle click toggles Do
// Not Disturb. Its icon carries the count itself, so the island shows no number beside it. With
// another backend, or before noti-fi's icon appears, the island is as follows.
// The Notifications island (spec §2.5, §2.10): the latest app's icon (else a bell), the count,
// a Do Not Disturb mark, and the latest summary on hover (or always with notif.show_summary),
// never while the screen is shared. While another app owns the notification name it says so.
// Left click opens the backend's centre (the built-in one hangs under this island); right click
// toggles Do Not Disturb. Everything shown comes from logic/notif.js islandView, which cleans it.
Island {
    id: root
    property var screen: null
    readonly property int iconSize: 18
    readonly property int maxText: 240
    // Read by the built-in Centre: its focus grab includes this bar so a second click toggles it shut.
    readonly property var barWindow: root.QsWindow.window
    readonly property bool hovered: mouse.containsMouse

    readonly property var view: N.islandView({
        count: NotifBackend.count,
        latest: NotifBackend.latest,
        dnd: NotifBackend.dnd,
        sharing: NotifBackend.sharing
    }, {
        hovered: root.hovered,
        showSummary: NotifBackend.showSummary,
        sharing: NotifBackend.sharing,
        yieldedTo: NotifBackend.yieldedTo
    })
    readonly property bool yielded: view.yieldedTo !== ""
    readonly property string countText: N.countLabel(view.count)
    readonly property string iconSource: {
        if (view.appIcon === "")
            return "";
        var r = L.iconRef(view.appIcon);
        if (r.path !== undefined)
            return r.path;
        return r.theme === L.DEFAULT_ICON ? "" : Quickshell.iconPath(r.theme, true);
    }
    readonly property bool open: Surfaces.open === "notifcentre" && Surfaces.anchor === root
    readonly property var notifiItem: Tray.find(SystemTray.items.values, Tray.NOTIFI)
    readonly property bool notifiBell: Tray.notifiBell(NotifBackend.backend, notifiItem)

    function openCentre() {
        if (!NotifBackend.available || !NotifBackend.hasCentre || root.yielded)
            return;
        if (NotifBackend.backend === "builtin")
            Surfaces.toggle("notifcentre", root.screen, root);
        else
            NotifBackend.toggleCentre();
    }
    function toggleDnd() {
        if (NotifBackend.available && !root.yielded)
            NotifBackend.setDnd(!NotifBackend.dnd);
    }

    // Sizes: Icon shows the bell (or the Do Not Disturb mark while it is on); Small adds the count
    // and the Do Not Disturb mark; Medium and Large add the summary (elided to fit).
    readonly property bool withSummary: size === "medium" || size === "large"
    color: open ? Theme.surfaceAlt : Theme.surface

    // The count and DnD state only: the summary is never read out.
    Accessible.role: Accessible.Button
    Accessible.name: root.yielded ? root.view.yieldedTo
        : "Notifications, " + (root.view.count > 0 ? root.countText : "none")
            + (root.view.dnd ? ", Do Not Disturb on" : "")
    Accessible.onPressAction: root.openCentre()

    RowLayout {
        id: row
        anchors.centerIn: parent
        width: Math.min(implicitWidth, root.room)
        spacing: 4

        // While another app owns the notification name.
        UqText {
            visible: root.yielded && !root.isIcon
            Layout.fillWidth: true
            text: root.view.yieldedTo
            color: Theme.textMuted
        }

        IconImage {
            visible: (!root.yielded || root.isIcon) && root.notifiBell && !(root.isIcon && root.view.dnd)
            implicitSize: root.iconSize
            Layout.preferredWidth: root.iconSize
            Layout.preferredHeight: root.iconSize
            source: root.notifiBell ? Net.safeIconSource(root.notifiItem.icon, "") : ""
        }
        IconImage {
            visible: (!root.yielded || root.isIcon) && !root.notifiBell && root.iconSource !== ""
                && !(root.isIcon && root.view.dnd)
            implicitSize: root.iconSize
            Layout.preferredWidth: root.iconSize
            Layout.preferredHeight: root.iconSize
            source: root.iconSource
        }
        UqText {
            visible: (!root.yielded || root.isIcon) && !root.notifiBell && root.iconSource === ""
                && !(root.isIcon && root.view.dnd)
            text: "\u{1F514}"
            color: NotifBackend.available ? Theme.text : Theme.textMuted
            font.pixelSize: Theme.fontSize + 3
        }
        // noti-fi's own icon already shows the count, so no second one beside it.
        UqText {
            visible: !root.yielded && !root.isIcon && !root.notifiBell && root.countText !== ""
            text: root.countText
            Layout.minimumWidth: implicitWidth
        }
        UqText {
            visible: (!root.yielded || root.isIcon) && root.view.dnd
            text: "\u{1F515}"
            color: Theme.textMuted
        }
        UqText {
            visible: !root.yielded && root.withSummary && root.view.summary !== ""
            Layout.fillWidth: true
            text: root.view.summary
            color: Theme.textMuted
        }
    }

    QsMenuAnchor {
        id: notifiMenu
        menu: root.notifiItem ? root.notifiItem.menu : null
        anchor.item: root
        anchor.edges: Edges.Bottom
        anchor.gravity: Edges.Bottom
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        onClicked: function (m) {
            if (root.notifiBell && !root.yielded) {
                if (m.button === Qt.MiddleButton)
                    root.toggleDnd();
                else if (m.button === Qt.RightButton || root.notifiItem.onlyMenu) {
                    if (root.notifiItem.hasMenu)
                        notifiMenu.open();
                } else
                    root.notifiItem.activate();
                return;
            }
            if (m.button === Qt.RightButton)
                root.toggleDnd();
            else if (m.button === Qt.LeftButton)
                root.openCentre();
        }
    }
}
