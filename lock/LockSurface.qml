import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import "core"
import "auth"
import "logic/lock.js" as L

// One lock surface per screen, laid out like the login screen (greeter/Greeter.qml): the login
// wallpaper (or uQ's backdrop), the clock at the top, and a card in the middle with the avatar,
// user name, password field, the PAM message and a fingerprint hint. All text is plain (UqText /
// UqInput). The backdrop, card, clock, avatar, field and messages are the shared auth/ components
// (auth/ is a symlink to ../shell/auth, like core/ and logic/).
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

    AuthBackdrop {
        anchors.fill: parent
        source: "file://" + L.WALLPAPER_PATH
    }

    Clock {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: Math.max(Theme.gapOut * 2, parent.height * 0.08)
        date: clock.date
    }

    AuthCard {
        id: card
        anchors.centerIn: parent
        width: Math.min(380, parent.width - Theme.gapOut * 2)
        height: form.implicitHeight + Theme.gapOut * 4
    }

    Column {
        id: form
        anchors.centerIn: card
        width: card.width - Theme.gapOut * 4
        spacing: Theme.gapOut

        Avatar {
            id: avatarBox
            anchors.horizontalCenter: parent.horizontalCenter
            size: 72
            border.color: Theme.accent
            avatar: surface.avatar
            userName: surface.userName
        }

        UqText {
            anchors.horizontalCenter: parent.horizontalCenter
            text: surface.userName
            Accessible.role: Accessible.StaticText
            Accessible.name: "User " + text
        }

        PasswordField {
            id: field
            width: parent.width
            busy: surface.auth.busy
            // Cleared after each attempt; kept while an earlier attempt is still being checked.
            onAccepted: {
                if (surface.auth.submit(field.text))
                    field.clear();
            }
            // Esc stops a check that PAM has not answered yet (Auth.cancel); the field keeps focus.
            Keys.onEscapePressed: event => {
                event.accepted = surface.auth.cancel();
            }
        }

        MessageView {
            width: parent.width
            message: surface.auth.message
        }

        MessageView {
            width: parent.width
            visible: surface.auth.fprintRunning
            label: "Fingerprint"
            message: surface.auth.fprintMessage
            fallback: "Or touch the fingerprint sensor"
        }
    }

    Component.onCompleted: field.forceActiveFocus()
}
