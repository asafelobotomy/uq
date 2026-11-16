import QtQuick
import Quickshell.Io

// File wiring for LauncherState: `<state>/uq/lookseedo.json`. Instantiate once (shell.qml).
Item {
    id: io

    property bool ready: false
    // Empty until Theme knows where $HOME is, so nothing is read or written at a made-up path.
    readonly property string path: ready ? Theme.uqDir + "/lookseedo.json" : ""

    // After ThemeIo has pointed Theme at $XDG_STATE_HOME.
    Component.onCompleted: Qt.callLater(function () { io.ready = true; })

    FileView {
        id: file
        path: io.path
        printErrors: false
        atomicWrites: true
        onLoaded: LauncherState.load(text())
        onLoadFailed: LauncherState.load("")
    }

    Connections {
        target: LauncherState
        function onSaveRequested(text) { file.setText(text); }
    }
}
