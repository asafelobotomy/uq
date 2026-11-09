//@ pragma UseQApplication
import QtQuick
import Quickshell
import Quickshell.Hyprland
import "core"
import "bar"
import "launcher"
import "osd"
import "notif"
import "notif/builtin"
import "session"

ShellRoot {
    // NotifBackend is a lazily created singleton: this binding touches it so it picks a backend
    // (and the built-in server registers) at startup, not when something first reads it. Keep it:
    // NotifBackend's Binding is what sets Builtin.wanted, so without this reference nothing would
    // create the singleton before something binds to Builtin.
    readonly property var notifBackend: NotifBackend
    // Session is lazy too: touch it at startup so its Bridge connections (LockState recovery,
    // PrepareForSleep, logind Lock) run before anything else uses it.
    readonly property var session: Session

    Component.onCompleted: Quickshell.watchFiles = false

    BridgeIo {}
    ThemeIo {}

    Connections {
        target: Hyprland
        function onRawEvent(event) { Bridge.handleRawEvent(event.name, event.data); }
    }

    Variants {
        model: Quickshell.screens
        delegate: Bar {}
    }

    // One Spotlight and one app menu (not per screen): they move to the focused monitor.
    Launchers {}

    // The volume, mic and brightness keys, their OSD source, and the fallback OSD window.
    OsdRoutes {}
    OsdWatch {}
    Osd {}

    // The built-in notification server's popups and centre (used when NotifBackend picks it).
    Popups {}
    Centre {}
}
