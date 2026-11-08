import QtQuick
import Quickshell
import Quickshell.Io

// File wiring for Theme. Instantiate once (shell.qml).
Item {
    id: io

    property bool ready: false

    // Re-reads by path each time: a fresh FileView per read avoids a stale inode watch.
    Component {
        id: readerC
        FileView {
            blockLoading: true
            printErrors: false
        }
    }

    // Watches uq/ (Phase 1 swaps theme/ as a whole, so the tokens.json inode goes stale).
    FileView {
        id: dirWatch
        path: io.ready ? Theme.uqDir : ""
        preload: false
        printErrors: false
        watchChanges: true
        onFileChanged: Theme.dirChanged()
    }

    FileView {
        id: tokensWatch
        path: io.ready ? Theme.tokensPath : ""
        preload: false
        printErrors: false
        watchChanges: true
        onFileChanged: Theme.dirChanged()
    }

    Connections {
        target: Bridge
        function onThemeChanged() { Theme.reload(); }
    }

    Component.onCompleted: {
        Theme.envLookup = function (k) { return Quickshell.env(k) || ""; };
        Theme.readText = function (path) {
            var v = readerC.createObject(io, { path: path });
            var t = null;
            try { t = v.text(); } catch (e) { t = null; }
            v.destroy();
            return t;
        };
        io.ready = true;
        Theme.reload();
    }
}
