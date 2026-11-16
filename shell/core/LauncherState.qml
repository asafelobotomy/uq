pragma Singleton
import QtQuick
import Quickshell.Io
import "../logic/lookhome.js" as H

// look=see>do_'s own memory: the five pinned squares and the apps opened most recently, as plain
// desktop ids in `<state>/uq/lookseedo.json`. Nothing here launches anything; logic/lookhome.js
// checks every id read back. A file that is missing or unreadable just means empty lists, and a
// save that fails leaves the lists as they are in memory for this session.
QtObject {
    id: root

    property var pins: H.normalisePins(null)
    property var recents: []
    property bool loaded: false

    // Empty until ThemeIo has told Theme where $HOME is, so nothing is read or written at a
    // made-up path.
    readonly property bool dirKnown: Theme.stateDir.charAt(0) === "/" && Theme.stateDir.indexOf("undefined") < 0
        && Theme.stateDir !== "/.local/state"
    readonly property string path: dirKnown ? Theme.uqDir + "/lookseedo.json" : ""

    function setPin(slot, id) {
        root.pins = H.setPin(root.pins, slot, id);
        root.save();
    }

    function recordRecent(id) {
        var next = H.pushRecent(root.recents, id);
        if (JSON.stringify(next) === JSON.stringify(root.recents))
            return;
        root.recents = next;
        root.save();
    }

    function save() {
        if (root.loaded)
            file.setText(H.serialise(root.pins, root.recents));
    }

    property FileView file: FileView {
        path: root.path
        printErrors: false
        atomicWrites: true
        onLoaded: {
            var s = H.parse(text());
            root.pins = s.pins;
            root.recents = s.recents;
            root.loaded = true;
        }
        onLoadFailed: root.loaded = true
    }
}
