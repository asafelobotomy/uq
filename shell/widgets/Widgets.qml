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
    readonly property var list: W.widgets(bridge ? bridge.widgets : [])
    readonly property var bar: W.barLayout(list)
    readonly property var quick: W.inSlot(list, "quick")
    readonly property var actions: W.appActions(bridge ? bridge.appActions : [])

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
            console.warn("uq: widget " + id + " unloaded: " + reason);
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
            console.warn("uq: app action " + action.id + ": " + e);
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
    }
}
