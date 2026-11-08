//@ pragma UseQApplication
import QtQuick
import Quickshell
import Quickshell.Hyprland
import "core"
import "bar"
import "launcher"

ShellRoot {
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
}
