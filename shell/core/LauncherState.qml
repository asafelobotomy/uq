pragma Singleton
import QtQuick
import "../logic/lookhome.js" as H

// look=see>do_'s own memory: the five pinned squares and the apps opened most recently, as plain
// desktop ids. Pure QML so it loads under qmltestrunner; LauncherStateIo (shell.qml) reads and
// writes the file. logic/lookhome.js checks every id read back, and a file that is missing or
// unreadable just means empty lists.
QtObject {
    id: root

    property var pins: H.normalisePins(null)
    property var recents: []
    // True once the file has been read (or found missing): nothing is saved before that.
    property bool loaded: false

    // The text LauncherStateIo should write; it listens for this.
    signal saveRequested(string text)

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
            root.saveRequested(H.serialise(root.pins, root.recents));
    }

    // Called by LauncherStateIo with the file's text, or "" when there is none.
    function load(text) {
        var s = H.parse(text);
        root.pins = s.pins;
        root.recents = s.recents;
        root.loaded = true;
    }
}
