import QtQuick
import Quickshell

// The one Spotlight and the one app menu, plus their shortcuts and IPC (LauncherRoutes). Which
// surface is open lives in Surfaces (it also closes and wipes both when Bridge.locked becomes
// true); nothing here keeps state of its own.
Scope {
    Spotlight {}
    AppMenu {}
    LauncherRoutes {}
}
