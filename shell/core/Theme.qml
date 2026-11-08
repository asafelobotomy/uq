pragma Singleton
import QtQuick
import "../logic/tokens.js" as Tokens

// Pure-QML so it loads under qmltestrunner; the file reading and watching live in ThemeIo.qml,
// which is instantiated once by shell.qml and injects `readText`.
QtObject {
    id: root

    property color bg: "#1f1f22"
    property color surface: "#2b2b2e"
    property color surfaceAlt: "#353538"
    property color overlay: "#464649"
    property color text: "#e4e4e7"
    property color textMuted: "#a9a9ac"
    property color accent: "#8ab4f8"
    property color accentText: "#1f1f22"
    property color border: "#5a5a5d"
    property color success: "#81c995"
    property color warning: "#fdd663"
    property color error: "#f28b82"
    property int radius: 10
    property int gapIn: 5
    property int gapOut: 12
    property int borderWidth: 2
    property string fontUi: "Inter"
    property string fontMono: "JetBrains Mono"
    property int fontSize: 11
    property string variant: "dark"

    // Test-only: replaces $XDG_STATE_HOME (default ~/.local/state).
    property string statePathOverride: ""
    // function(path) -> string | null; injected by ThemeIo (or a test).
    property var readText: null

    readonly property string stateDir: statePathOverride !== "" ? statePathOverride
        : (env("XDG_STATE_HOME") || (env("HOME") + "/.local/state"))
    readonly property string uqDir: stateDir + "/uq"
    readonly property string tokensPath: uqDir + "/theme/tokens.json"

    // Set by ThemeIo from Quickshell.env; empty when not running under qs.
    property var envLookup: function (k) { return ""; }
    function env(k) { return envLookup(k); }

    function apply(obj) {
        var t = Tokens.toTheme(obj);
        for (var k in t)
            if (root[k] !== t[k])
                root[k] = t[k];
    }

    // Reads tokens.json by path every time (never a held handle), so a swapped theme/ directory
    // is picked up. A failed read or parse keeps the previous values.
    function reload() {
        if (!readText)
            return;
        var text = null;
        try {
            text = readText(tokensPath);
        } catch (e) {
            text = null;
        }
        var o = Tokens.parse(text);
        if (o !== null)
            apply(o);
    }

    // Parent-directory watch backstop: debounced 2 s, then reload.
    function dirChanged() {
        debounce.restart();
    }

    property Timer debounce: Timer {
        interval: 2000
        onTriggered: root.reload()
    }
}
