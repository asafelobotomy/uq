pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "../logic/clip.js" as C

// The clipboard history as the shell sees it: uqlip's list, kept live by `uqctl clip watch`
// (one snapshot per change), and the actions on it. The popup and the bar island both read this.
// When uqlip isn't running the watch ends and is tried again after 5, 10, 30 and then every 60 s
// (a spawn each time, so a stopped uqlip costs next to nothing); nothing here blocks the shell.
Singleton {
    id: root

    property var entries: []
    property int count: 0
    property bool paused: false
    property bool enabled: true
    property bool connected: false
    // True while the popup is up (set by toggle/open/close, which the shortcut, IPC and island use).
    property bool open: false

    function toggle() { open = !open; }
    function show() { open = true; }
    function hide() { open = false; }

    // `uqctl clip <args>` in the background; the list is read again when it ends.
    function _run(args) {
        Quickshell.execDetached(["uqctl", "clip"].concat(args));
    }
    // Put entry `id` on the clipboard and close.
    function copy(id) { _run(["copy", String(id)]); open = false; }
    function pin(id, pinned) { _run([pinned ? "pin" : "unpin", String(id)]); }
    function remove(id) { _run(["delete", String(id)]); }
    function clear() { _run(["clear"]); }
    function setPaused(p) { _run([p ? "pause" : "resume"]); }

    function refresh() {
        if (!lister.running)
            lister.running = true;
        else
            again = true;
    }
    property bool again: false

    // Every change the daemon announces (a copy, a pin, a delete, pause...) reads the list again.
    Process {
        id: lister
        command: ["uqctl", "clip", "list", "--json", "--limit", "200"]
        stdout: StdioCollector {
            onStreamFinished: {
                var r = C.parseList(text);
                if (r !== null)
                    root.entries = r;
            }
        }
        onRunningChanged: {
            if (!running && root.again) {
                root.again = false;
                lister.running = true;
            }
        }
    }

    Process {
        id: watcher
        command: ["uqctl", "clip", "watch"]
        running: true
        stdout: SplitParser {
            onRead: data => {
                var s = C.parseState(data);
                if (s === null)
                    return;
                root.paused = s.paused;
                root.enabled = s.enabled;
                root.count = s.count;
                root.connected = true;
                root.misses = 0;
                root.refresh();
            }
        }
        onRunningChanged: {
            if (!running) {
                root.connected = false;
                retry.interval = C.retryDelayMs(root.misses);
                root.misses += 1;
                retry.restart();
            }
        }
    }
    // Watches that ended without a snapshot since the last one that had one.
    property int misses: 0
    Timer {
        id: retry
        interval: 5000
        onTriggered: watcher.running = true
    }
}
