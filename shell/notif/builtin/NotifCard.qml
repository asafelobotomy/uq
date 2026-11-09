pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets
import "../../core"
import ".."
import "../../logic/launcher.js" as L
import "../../logic/sanitize.js" as S

// One built-in notification (a popup or a centre row). Its content is untrusted: everything
// shown comes from sanitize.js popupView(), so while the screen is shared only the app name and
// icon show (and that follows NotifBackend.sharing live). Text is UqText; the body goes only
// through NotifBody. Clicking the card runs the notification's "default" action if it has one;
// the close button dismisses it.
Rectangle {
    id: root

    required property var notification          // Quickshell Notification
    property real dpr: 1                         // the window's screen.devicePixelRatio

    readonly property bool sharing: NotifBackend.sharing
    readonly property var view: S.popupView(notification, sharing)
    readonly property string imageSource: sharing || !notification ? "" : S.imageRef(notification.image)
    readonly property bool hasDefault: {
        var list = notification ? notification.actions || [] : [];
        for (var i = 0; i < list.length; i++)
            if (list[i] && list[i].identifier === "default")
                return true;
        return false;
    }
    readonly property int iconSize: 24
    readonly property int imageSize: 64

    function iconSource(icon) {
        var r = L.iconRef(icon);
        if (r.path !== undefined)
            return r.path;
        return Quickshell.iconPath(r.theme, true) || Quickshell.iconPath(L.DEFAULT_ICON, true);
    }

    function activate() {
        if (root.hasDefault)
            Builtin.invoke(root.notification, "default");
    }

    implicitHeight: layout.implicitHeight + Theme.gapOut * 2
    radius: Theme.radius
    color: Theme.surface
    border.color: Theme.border
    border.width: Theme.borderWidth
    Accessible.role: root.hasDefault ? Accessible.Button : Accessible.Pane
    Accessible.name: [root.view.appName, root.view.summary, S.plainBody(root.view.body)]
        .filter(function (s) { return s !== ""; }).join(", ")
    Accessible.onPressAction: root.activate()

    // A small text button (actions, close).
    component CardButton: Rectangle {
        id: b
        property string label: ""
        property string accessibleName: label
        signal activated()

        implicitHeight: 24
        implicitWidth: Math.min(160, bText.implicitWidth + Theme.gapIn * 4)
        radius: Theme.radius - 4
        color: bMouse.containsMouse ? Theme.overlay : Theme.surfaceAlt
        activeFocusOnTab: true
        border.color: Theme.accent
        border.width: activeFocus ? 1 : 0
        Accessible.role: Accessible.Button
        Accessible.name: b.accessibleName
        Accessible.onPressAction: b.activated()
        Keys.onSpacePressed: b.activated()
        Keys.onReturnPressed: b.activated()

        UqText {
            id: bText
            anchors.fill: parent
            anchors.leftMargin: Theme.gapIn * 2
            anchors.rightMargin: Theme.gapIn * 2
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            text: b.label
        }
        MouseArea {
            id: bMouse
            anchors.fill: parent
            hoverEnabled: true
            onClicked: b.activated()
        }
    }

    // Under the content, so links in the body still get their hover.
    MouseArea {
        anchors.fill: parent
        cursorShape: root.hasDefault ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.activate()
    }

    RowLayout {
        id: layout
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: Theme.gapOut
        spacing: Theme.gapIn * 2

        IconImage {
            Layout.alignment: Qt.AlignTop
            Layout.preferredWidth: root.iconSize
            Layout.preferredHeight: root.iconSize
            implicitSize: root.iconSize
            backer.sourceSize.width: root.iconSize * root.dpr
            backer.sourceSize.height: root.iconSize * root.dpr
            source: root.iconSource(root.view.appIcon)
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignTop
            spacing: Theme.gapIn

            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.gapIn
                UqText {
                    Layout.fillWidth: true
                    text: root.view.appName
                    color: Theme.textMuted
                }
                CardButton {
                    label: "✕"
                    accessibleName: "Dismiss notification"
                    onActivated: Builtin.dismiss(root.notification)
                }
            }

            UqText {
                Layout.fillWidth: true
                visible: text !== ""
                text: root.view.summary
                font.bold: true
                wrapMode: Text.Wrap
                maximumLineCount: 2
            }

            NotifBody {
                Layout.fillWidth: true
                visible: raw !== ""
                raw: root.view.body
            }

            Flow {
                Layout.fillWidth: true
                visible: root.view.actions.length > 0
                spacing: Theme.gapIn
                Repeater {
                    model: root.view.actions
                    delegate: CardButton {
                        required property var modelData
                        label: modelData.text
                        accessibleName: modelData.text !== "" ? modelData.text : "Action"
                        onActivated: Builtin.invoke(root.notification, modelData.identifier)
                    }
                }
            }
        }

        Image {
            Layout.alignment: Qt.AlignTop
            Layout.preferredWidth: root.imageSize
            Layout.preferredHeight: root.imageSize
            visible: root.imageSource !== ""
            source: root.imageSource
            asynchronous: true
            fillMode: Image.PreserveAspectFit
            sourceSize.width: root.imageSize * root.dpr
            sourceSize.height: root.imageSize * root.dpr
            Accessible.role: Accessible.Graphic
            Accessible.name: "Notification image"
        }
    }
}
