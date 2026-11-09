import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets
import "../core"
import "../notif"
import "../logic/notif.js" as N
import "../logic/launcher.js" as L

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

    implicitWidth: row.implicitWidth + Theme.gapIn * 2
    color: open ? Theme.surfaceAlt : Theme.surface

    // The count and DnD state only: the summary is never read out.
    Accessible.role: Accessible.Button
    Accessible.name: root.yielded ? root.view.yieldedTo
        : "Notifications, " + (root.view.count > 0 ? root.countText : "none")
            + (root.view.dnd ? ", Do Not Disturb on" : "")
    Accessible.onPressAction: root.openCentre()

    RowLayout {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        spacing: 4

        // While another app owns the notification name.
        UqText {
            visible: root.yielded
            Layout.maximumWidth: root.maxText
            text: root.view.yieldedTo
            color: Theme.textMuted
        }

        IconImage {
            visible: !root.yielded && root.iconSource !== ""
            implicitSize: root.iconSize
            Layout.preferredWidth: root.iconSize
            Layout.preferredHeight: root.iconSize
            source: root.iconSource
        }
        UqText {
            visible: !root.yielded && root.iconSource === ""
            text: "\u{1F514}"
            color: NotifBackend.available ? Theme.text : Theme.textMuted
            font.pixelSize: Theme.fontSize + 3
        }
        UqText {
            visible: !root.yielded && root.countText !== ""
            text: root.countText
        }
        UqText {
            visible: !root.yielded && root.view.dnd
            text: "\u{1F515}"
            color: Theme.textMuted
        }
        UqText {
            visible: !root.yielded && root.view.summary !== ""
            Layout.maximumWidth: root.maxText
            text: root.view.summary
            color: Theme.textMuted
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: function (m) {
            if (m.button === Qt.RightButton)
                root.toggleDnd();
            else
                root.openCentre();
        }
    }
}
