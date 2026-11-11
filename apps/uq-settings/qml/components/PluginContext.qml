// The `ctx` a plugin page receives (Phase 4 spec §2.3; docs/settings-pages.md):
//   ctx.theme                                   the design tokens (tokens.json)
//   ctx.command.run(id, args, callback)         a [[command]] of the page; callback gets
//                                               {code, output} or {error, …}
//   ctx.dbus.call(method, args, callback[, target])
//   ctx.dbus.get(property, callback[, target])  callback gets {value} or {error}
//   ctx.openApp(desktopId[, callback])          open an app ("name.desktop") as the launcher
//                                               does; callback gets {ok: true} or {error}
// `target` is {path, interface}, needed only by a page without a [dbus] table.
// Everything goes through `host` (the PluginHost singleton); without one every request fails
// with an error. A command with `confirm` raises confirmNeeded; the page shows the dialog and
// calls answer(). Free of Kirigami so it can be tested on its own.
import QtQuick

QtObject {
    id: context

    /// The PluginHost singleton (or a stand-in in tests); null makes every request fail.
    property var host: null
    /// The page's id, as in page.toml.
    property string pageId
    property var theme: ({})
    /// request id -> callback, for this page's requests only.
    property var callbacks: ({})

    /// A command of this page asks to be confirmed: show `text` and call answer(request, …).
    signal confirmNeeded(int request, string text)

    readonly property var ctx: ({
            theme: context.theme,
            command: {
                run: (id, args, callback) => context.run(id, args, callback)
            },
            dbus: {
                call: (method, args, callback, target) => context.call(method, args, callback, target),
                get: (property, callback, target) => context.get(property, callback, target)
            },
            openApp: (desktopId, callback) => context.openApp(desktopId, callback)
        })

    function finish(callback, result) {
        if (typeof callback !== "function")
            return;
        try {
            callback(result);
        } catch (e) {
            console.warn("uq-settings: page " + context.pageId + ": its callback failed: " + e);
        }
    }

    function track(request, callback) {
        context.callbacks[request] = callback;
        return request;
    }

    function unavailable(callback) {
        // A fresh function each time: Qt.callLater merges repeated calls of the same one.
        Qt.callLater(() => context.finish(callback, {
                    error: "This page can't do that here."
                }));
        return 0;
    }

    function targetJson(target) {
        return target ? JSON.stringify(target) : "";
    }

    function run(id, args, callback) {
        if (!context.host)
            return context.unavailable(callback);
        return context.track(context.host.runCommand(context.pageId, String(id), JSON.stringify(args || [])), callback);
    }

    function call(method, args, callback, target) {
        if (!context.host)
            return context.unavailable(callback);
        return context.track(context.host.dbusCall(context.pageId, String(method), JSON.stringify(args || []), context.targetJson(target)), callback);
    }

    function get(property, callback, target) {
        if (!context.host)
            return context.unavailable(callback);
        return context.track(context.host.dbusGet(context.pageId, String(property), context.targetJson(target)), callback);
    }

    function openApp(desktopId, callback) {
        if (!context.host)
            return context.unavailable(callback);
        return context.track(context.host.openApp(context.pageId, String(desktopId)), callback);
    }

    /// The person's answer to confirmNeeded(request, …).
    function answer(request, confirmed) {
        if (context.host && request in context.callbacks)
            context.host.answer(request, confirmed);
    }

    property Connections hostConnections: Connections {
        target: context.host
        ignoreUnknownSignals: true

        function onFinished(request, resultJson) {
            if (!(request in context.callbacks))
                return;
            const callback = context.callbacks[request];
            delete context.callbacks[request];
            let result;
            try {
                result = JSON.parse(resultJson);
            } catch (e) {
                result = {
                    error: "The answer couldn't be read."
                };
            }
            context.finish(callback, result);
        }

        function onConfirmRequested(request, page, text) {
            if (request in context.callbacks)
                context.confirmNeeded(request, text);
        }
    }
}
