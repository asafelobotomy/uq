import QtQuick
import Quickshell
import "../core"
import "../logic/welcome.js" as W

// Starts Welcome (`uq-settings --welcome`) once, when uqd says it is due and no installer is
// holding it back; logic/welcome.js has the rules.
QtObject {
    id: root

    property var state: W.initial()

    function act(r) {
        root.state = r.state;
        if (r.action === "start") {
            Quickshell.execDetached(["uq-settings", "--welcome"]);
        } else if (r.action === "wait") {
            grace.restart();
        } else if (r.action === "ask") {
            Bridge.call("WelcomeStatus", []).then(function (s) {
                root.act(W.onStatus(root.state, s));
            }).catch(function () {
                root.act(W.onStatus(root.state, null));
            });
        }
    }

    property Timer grace: Timer {
        interval: W.GRACE_MS
        onTriggered: root.act(W.onTimer(root.state))
    }

    property Connections conn: Connections {
        target: Bridge
        function onEvent(name, data) {
            root.act(W.onEvent(root.state, name, data));
        }
    }
}
