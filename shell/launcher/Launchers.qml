import QtQuick
import Quickshell
import "../quick"
import "../power"

// The one look=see>do_, the one app menu, the one quick-settings panel and the power menu, plus
// their shortcuts and IPC (LauncherRoutes). Which surface is open lives in Surfaces (it also closes and wipes them
// when Bridge.locked becomes true); nothing here keeps state of its own.
Scope {
    // Built in the background once the shell is up, not before the first bar shows (they took
    // about an eighth of startup). Each opens from Surfaces.open, so one asked for while this
    // is still loading opens as soon as it exists.
    LazyLoader {
        activeAsync: true
        Scope {
            LookSeeDo {}
            AppMenu {}
            QuickSettings {}
            PowerMenu {}
        }
    }
    LauncherRoutes {}
}
