import QtQuick
import "../core"

// One widget in a fixed-size slot (Phase 4 spec §2.2). The widget's QML is loaded with a Loader
// and handed `ctx` (WidgetContext.api). A load error, or an exception from a callback the widget
// gave ctx, unloads it; Widgets logs that once per session, and the slot tries again at most
// once a minute. An unloaded slot takes no room.
Item {
    id: slot

    // An entry of Widgets.list: {id, name, slot, order, qmlUrl, visible}.
    property var widget: null
    property string kind: "bar"
    property var registry: Widgets
    property var bridge: Bridge
    // The cell size: the bar's 1-cell icon, or a quick-settings row.
    property int cell: 26
    property int rowHeight: 32

    readonly property bool loaded: loader.status === Loader.Ready && loader.item !== null
    // An unloaded slot takes no room.
    readonly property bool shown: loaded
    readonly property var item: loaded ? loader.item : null
    // The widget's optional `popup` Component (bar popup or quick detail view).
    readonly property var popup: item && item.popup !== undefined ? item.popup : null
    readonly property var ctx: context.api
    property string error: ""
    property string loadedUrl: ""

    signal popupRequested(bool open)

    implicitWidth: kind === "bar" ? cell : 200
    implicitHeight: kind === "bar" ? cell : rowHeight
    width: kind === "bar" ? cell : implicitWidth
    height: kind === "bar" ? cell : rowHeight
    clip: true
    visible: shown

    WidgetContext {
        id: context
        widgetId: slot.widget ? slot.widget.id : ""
        slot: slot.kind
        registry: slot.registry
        bridge: slot.bridge
        onFailed: function (reason) { slot.fail(reason); }
        onPopupRequested: function (open) { slot.popupRequested(open); }
    }

    Loader {
        id: loader
        anchors.fill: parent
        onStatusChanged: {
            if (status === Loader.Error)
                slot.fail("couldn't load " + slot.loadedUrl);
        }
    }

    Timer {
        id: retry
        onTriggered: slot.load()
    }
    Timer {
        id: unloadSoon
        interval: 0
        onTriggered: slot.unload()
    }

    function load() {
        retry.stop();
        unloadSoon.stop();
        if (!widget) {
            unload();
            return;
        }
        if (!registry.mayLoad(widget.id)) {
            unload();
            retry.interval = Math.max(1, registry.retryIn(widget.id));
            retry.start();
            return;
        }
        error = "";
        loadedUrl = widget.qmlUrl;
        context.reset();
        loader.setSource(widget.qmlUrl, { ctx: context.api });
    }

    function unload() {
        context.stop();
        if (loader.source != "")
            loader.source = "";
        popupRequested(false);
    }

    function fail(reason) {
        if (!widget)
            return;
        error = String(reason);
        registry.failed(widget.id, error);
        context.stop();
        // Never destroy the item from inside its own call stack.
        unloadSoon.restart();
        retry.interval = Math.max(1, registry.retryIn(widget.id));
        retry.start();
    }

    onWidgetChanged: {
        if (!widget)
            unload();
        else if (widget.qmlUrl !== loadedUrl || !loaded)
            load();
    }
    Component.onCompleted: if (widget && !loaded && loadedUrl === "") load()
    Component.onDestruction: context.stop()
}
