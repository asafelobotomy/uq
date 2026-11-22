pragma Singleton
import QtQuick
import "../core"
import "../logic/widgets.js" as W

// Widgets and app actions from other apps (Phase 4 spec §2.2, §2.4.2): the cleaned-up lists the
// bridge sent, the load-failure registry shared by every slot (a widget that fails is logged
// once per session and tried again at most once a minute), and the `WidgetProps` /
// `WidgetSignal` events, per widget. Pure QML: loads under qmltestrunner.
QtObject {
    id: root

    // The bridge link; tests swap in a fake with the same `event` signal and `call`.
    property var bridge: Bridge
    // The bridge's list as sent, and as slots load it: a widget whose files changed gets a new
    // spelling of its URL, so Qt compiles it again (see W.nextRevs).
    readonly property var sent: W.widgets(bridge ? bridge.widgets : [])
    property var revs: ({})
    property var list: []
    function respell() {
        revs = W.nextRevs(revs, sent);
        list = W.withRevs(sent, revs);
    }
    onSentChanged: respell()
    Component.onCompleted: respell()
    readonly property var bar: W.barLayout(list)
    readonly property var quick: W.inSlot(list, "quick")
    readonly property var actions: W.appActions(bridge ? bridge.appActions : [])

    // How many slots of each widget are on screen, and what the bridge was last told: while
    // none is, the bridge drops the widget's D-Bus subscription (no WidgetProps), and sends the
    // current values when one is again. The bridge takes an unreported widget as shown.
    property var shownCount: ({})
    property var reported: ({})
    function slotShown(id, on) {
        var c = Object.assign({}, shownCount);
        c[id] = Math.max(0, (c[id] || 0) + (on ? 1 : -1));
        shownCount = c;
        Qt.callLater(syncShown);
    }
    function syncShown() {
        if (!bridge || !bridge.connected)
            return;
        var r = Object.assign({}, reported), ids = list.map(function (w) { return w.id; });
        for (var i = 0; i < ids.length; i++) {
            var id = ids[i], want = (shownCount[id] || 0) > 0;
            if ((r[id] === undefined ? true : r[id]) === want)
                continue;
            r[id] = want;
            bridge.call("WidgetShown", [id, want]).catch(function () {});
        }
        reported = r;
    }
    onListChanged: Qt.callLater(syncShown)

    property var failures: W.failures()
    // Every failure reason logged so far, newest last (for tests and debugging).
    property var log: []

    signal props(string widget, var props)
    signal widgetSignal(string widget, string member, var args)

    function now() {
        return Date.now();
    }

    function mayLoad(id) {
        return W.mayLoad(failures, id, now());
    }

    function retryIn(id) {
        return W.retryIn(failures, id, now());
    }

    // A widget failed to load or threw: logged once per session; the slot unloads it.
    function failed(id, reason) {
        var reg = failures;
        if (W.noteFailure(reg, id, now())) {
            console.warn("uq[WIDGET-001] widget " + id + " unloaded: " + reason);
            var l = log.slice();
            l.push(id + ": " + reason);
            log = l;
        }
        failures = reg;
    }

    // App actions for a launcher row.
    function actionsFor(appId) {
        return W.actionsFor(actions, appId);
    }

    // Run an app action on a row; the bridge builds the command line.
    function runAction(action, appId) {
        var c = W.actionCall(action, appId);
        if (c === null || !bridge || Bridge.locked)
            return false;
        bridge.call(c.call, c.args).catch(function (e) {
            console.warn("uq[WIDGET-002] app action " + action.id + ": " + e);
        });
        return true;
    }

    function handle(name, data) {
        if (!data || typeof data !== "object" || typeof data.widget !== "string")
            return;
        if (name === "WidgetProps" && data.props && typeof data.props === "object")
            props(data.widget, data.props);
        else if (name === "WidgetSignal" && typeof data.member === "string")
            widgetSignal(data.widget, data.member, W.toList(data.args));
    }

    property Connections conn: Connections {
        target: root.bridge
        ignoreUnknownSignals: true
        function onEvent(name, data) { root.handle(name, data); }
        // A new bridge takes every widget as shown again.
        function onConnectedChanged() {
            root.reported = {};
            Qt.callLater(root.syncShown);
        }
    }
}
