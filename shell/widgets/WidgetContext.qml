import QtQuick
import "../core"
import "../logic/widgets.js" as W

// The single `ctx` a widget's root item receives (Phase 4 spec §2.2), as `api`:
//
//   ctx.Theme (also ctx.theme)   the design tokens (the shell's Theme)
//   ctx.UqText, ctx.UqInput      Components: the shell's plain-text label and input
//   ctx.slot                     "bar" or "quick"
//   ctx.dbus.call(method, args, cb)   cb({value} | {error}); only methods listed in [dbus]
//   ctx.dbus.get(cb)                  every listed property: cb({value: {...}} | {error})
//   ctx.dbus.props                    the listed properties, kept current from PropertiesChanged
//   ctx.dbus.onProps(cb)              cb(changed) on each update
//   ctx.dbus.onSignal(cb)             cb(member, args) for each listed signal
//   ctx.command.run(id, args, cb)     cb({code, output} | {code, error, output} | {error})
//   ctx.openPopup(), ctx.closePopup() the widget's `popup` Component (bar: a popup under the
//                                     icon; quick: a detail view under the tile)
//   ctx.fail(reason)                  give up: the slot unloads the widget
//
// No Quickshell.Io and no bridge: every request goes through the bridge's widget calls, which
// enforce the manifest. An exception thrown by a callback above unloads the widget.
QtObject {
    id: ctx

    property string widgetId: ""
    property string slot: "bar"
    property var registry: Widgets
    property var bridge: Bridge
    // Bumped on every (re)load: answers for an older load are dropped.
    property int generation: 0
    property bool alive: false

    signal failed(string reason)
    signal popupRequested(bool open)

    readonly property Component uqText: Component {
        UqText {}
    }
    readonly property Component uqInput: Component {
        UqInput {}
    }

    // Start a fresh load: no handlers, no props, older answers ignored.
    function reset() {
        generation++;
        dbus.props = ({});
        dbus.propHandlers = [];
        dbus.signalHandlers = [];
        alive = true;
    }

    function stop() {
        generation++;
        alive = false;
        dbus.propHandlers = [];
        dbus.signalHandlers = [];
    }

    function fail(reason) {
        if (!alive)
            return;
        alive = false;
        failed(String(reason));
    }

    // Call into widget code. A throw there is a runtime error: the widget is unloaded.
    function deliver(gen, cb, a, b) {
        if (!alive || gen !== generation || typeof cb !== "function")
            return;
        try {
            if (b === undefined)
                cb(a);
            else
                cb(a, b);
        } catch (e) {
            fail(String(e));
        }
    }

    function request(call, args, cb, wrap) {
        var gen = generation;
        if (!bridge || !alive) {
            Qt.callLater(function () { ctx.deliver(gen, cb, { error: "not available" }); });
            return;
        }
        bridge.call(call, args).then(function (v) {
            ctx.deliver(gen, cb, wrap ? { value: v } : v);
        }).catch(function (e) {
            ctx.deliver(gen, cb, { error: String(e) });
        });
    }

    readonly property QtObject dbus: QtObject {
        property var props: ({})
        property var propHandlers: []
        property var signalHandlers: []

        function call(method, args, cb) {
            ctx.request("WidgetCall", [ctx.widgetId, String(method), Array.isArray(args) ? args : []],
                        cb, true);
        }
        function get(cb) {
            var gen = ctx.generation;
            ctx.request("WidgetGet", [ctx.widgetId], function (r) {
                if (gen === ctx.generation && r && r.value && typeof r.value === "object")
                    ctx.dbus.props = W.mergeProps(ctx.dbus.props, r.value);
                if (typeof cb === "function")
                    cb(r);
            }, true);
        }
        function onProps(cb) {
            if (typeof cb === "function")
                propHandlers = propHandlers.concat([cb]);
        }
        function onSignal(cb) {
            if (typeof cb === "function")
                signalHandlers = signalHandlers.concat([cb]);
        }
    }

    readonly property QtObject command: QtObject {
        function run(id, args, cb) {
            var list = Array.isArray(args) ? args.map(function (a) { return String(a); }) : [];
            ctx.request("WidgetRun", [ctx.widgetId, String(id), list], cb, false);
        }
    }

    // What the widget sees as `ctx`. A plain object, so it can carry Theme, UqText and UqInput
    // under the names the spec gives them.
    readonly property var api: ({
        Theme: Theme,
        theme: Theme,
        UqText: ctx.uqText,
        UqInput: ctx.uqInput,
        slot: ctx.slot,
        dbus: ctx.dbus,
        command: ctx.command,
        openPopup: function () { if (ctx.alive) ctx.popupRequested(true); },
        closePopup: function () { ctx.popupRequested(false); },
        fail: function (reason) { ctx.fail(reason === undefined ? "the widget gave up" : reason); }
    })

    property Connections events: Connections {
        target: ctx.registry
        ignoreUnknownSignals: true
        function onProps(widget, props) {
            if (widget !== ctx.widgetId || !ctx.alive)
                return;
            ctx.dbus.props = W.mergeProps(ctx.dbus.props, props);
            var hs = ctx.dbus.propHandlers, gen = ctx.generation;
            for (var i = 0; i < hs.length; i++)
                ctx.deliver(gen, hs[i], props);
        }
        function onWidgetSignal(widget, member, args) {
            if (widget !== ctx.widgetId || !ctx.alive)
                return;
            var hs = ctx.dbus.signalHandlers, gen = ctx.generation;
            for (var i = 0; i < hs.length; i++)
                ctx.deliver(gen, hs[i], member, args);
        }
    }
}
