import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import "core"
import "logic/lock.js" as L

// One lock surface per screen: clock, date, avatar, user name, password field, the PAM message and
// a fingerprint hint. All text is plain (UqText / UqInput).
WlSessionLockSurface {
    id: surface

    required property var auth

    readonly property string userName: Quickshell.env("USER") || ""
    readonly property string home: Quickshell.env("HOME") || ""
    readonly property string accountsIconPath: userName !== "" ? "/var/lib/AccountsService/icons/" + userName : ""
    property bool accountsIconExists: false
    property bool faceExists: false
    readonly property var avatar: L.avatarSource(accountsIconExists ? accountsIconPath : "", faceExists,
                                                 home, userName)

    color: Theme.bg

    // Existence checks only: the bytes are never shown from here (Image reads the file itself).
    FileView {
        path: surface.accountsIconPath
        printErrors: false
        onLoaded: surface.accountsIconExists = true
        onLoadFailed: surface.accountsIconExists = false
    }
    FileView {
        path: surface.home !== "" ? surface.home + "/.face" : ""
        printErrors: false
        onLoaded: surface.faceExists = true
        onLoadFailed: surface.faceExists = false
    }

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
    }

    Column {
        anchors.centerIn: parent
        width: Math.min(360, parent.width - Theme.gapOut * 2)
        spacing: Theme.gapOut

        UqText {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatDateTime(clock.date, "HH:mm")
            font.pixelSize: Theme.fontSize * 6
            Accessible.role: Accessible.StaticText
            Accessible.name: "Time " + text
        }
        UqText {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatDateTime(clock.date, "dddd d MMMM")
            font.pixelSize: Theme.fontSize * 1.5
            color: Theme.textMuted
            Accessible.role: Accessible.StaticText
            Accessible.name: "Date " + text
        }

        Item { width: 1; height: Theme.gapOut }

        Rectangle {
            id: avatarBox
            anchors.horizontalCenter: parent.horizontalCenter
            width: 96
            height: 96
            radius: width / 2
            color: Theme.surfaceAlt
            border.color: Theme.border
            border.width: Theme.borderWidth
            clip: true
            Accessible.role: Accessible.Graphic
            Accessible.name: "Avatar for " + surface.userName

            Image {
                anchors.fill: parent
                anchors.margins: Theme.borderWidth
                visible: surface.avatar.path !== undefined && status === Image.Ready
                source: surface.avatar.path !== undefined ? surface.avatar.path : ""
                sourceSize.width: 192
                sourceSize.height: 192
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                cache: false
            }
            UqText {
                anchors.centerIn: parent
                visible: surface.avatar.initial !== undefined
                text: surface.avatar.initial !== undefined ? surface.avatar.initial : ""
                font.pixelSize: 40
                Accessible.ignored: true
            }
        }

        UqText {
            anchors.horizontalCenter: parent.horizontalCenter
            text: surface.userName
            font.pixelSize: Theme.fontSize * 1.4
            Accessible.role: Accessible.StaticText
            Accessible.name: "User " + text
        }

        UqInput {
            id: field
            width: parent.width
            implicitHeight: 36
            secret: true
            placeholder: surface.auth.busy ? "Checking…" : "Password"
            Accessible.role: Accessible.EditableText
            Accessible.name: "Password"
            onAccepted: {
                surface.auth.submit(field.text);
                field.clear();
            }
        }

        UqText {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            elide: Text.ElideNone
            visible: text !== ""
            text: surface.auth.message.text
            color: surface.auth.message.error ? Theme.error : Theme.textMuted
            Accessible.role: Accessible.StaticText
            Accessible.name: "Message " + text
        }

        UqText {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            elide: Text.ElideNone
            visible: surface.auth.fprintRunning
            text: surface.auth.fprintMessage.text !== "" ? surface.auth.fprintMessage.text
                                                         : "Or touch the fingerprint sensor"
            color: surface.auth.fprintMessage.error ? Theme.error : Theme.textMuted
            Accessible.role: Accessible.StaticText
            Accessible.name: "Fingerprint " + text
        }
    }

    Component.onCompleted: field.forceActiveFocus()
}
