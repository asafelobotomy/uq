import QtQuick
import Quickshell
import "../quick"

// The one Spotlight, the one app menu and the one quick-settings panel, plus their shortcuts and
// IPC (LauncherRoutes). Which surface is open lives in Surfaces (it also closes and wipes them
// when Bridge.locked becomes true); nothing here keeps state of its own.
Scope {
    Spotlight {}
    AppMenu {}
    QuickSettings {}
    LauncherRoutes {}
}
