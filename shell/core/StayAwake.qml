pragma Singleton
import QtQuick

// Stay awake (quick settings): while enabled, each bar's IdleInhibitor blocks every idle stage
// (spec §2.7). Session-only, never persisted; pure QML so core stays free of Quickshell imports.
QtObject {
    property bool enabled: false
}
