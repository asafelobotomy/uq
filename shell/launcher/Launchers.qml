import QtQuick
import Quickshell
import "../quick"
import "../power"

// The one Spotlight, the one app menu, the one quick-settings panel and the power menu, plus
// their shortcuts and IPC (LauncherRoutes). Which surface is open lives in Surfaces (it also closes and wipes them
// when Bridge.locked becomes true); nothing here keeps state of its own.
Scope {
    Spotlight {}
    AppMenu {}
    QuickSettings {}
    PowerMenu {}
    LauncherRoutes {}
}
