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

ShellRoot {
    // NotifBackend is a lazily created singleton: this binding touches it so it picks a backend
    // (and the built-in server registers) at startup, not when something first reads it.
    readonly property var notifBackend: NotifBackend

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
