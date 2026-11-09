import QtQuick
import Quickshell
import Quickshell.Services.Pipewire
import "../brightness"
import "../logic/quick.js" as Q

// Window-free OSD source (safe to create headless). Volume and mute changes on the default sink
// and source go to OsdRouter, gated by osdShouldShow: nothing in the first 2 s, and nothing for a
// default-device switch. On a switch (and at startup) the node id to compare against is cleared,
// and recorded again only once the new node is bound and a settle delay has passed, so the
// signals its binding emits show nothing and the first real key press after that does.
// Brightness OSDs come from Brightness.changed.
Scope {
    id: root

    readonly property int settleMs: 400
    property real startedAt: NaN
    property var prevSinkId: null
    property var prevSourceId: null
    readonly property PwNode sink: Pipewire.defaultAudioSink
    readonly property PwNode source: Pipewire.defaultAudioSource

    // One level change on `node`, compared against the settled id `prev`. Never writes prev.
    function level(kind, node, prev) {
        if (!node || !node.audio)
            return;
        if (Q.osdShouldShow(startedAt, Date.now(), prev, node.id))
            OsdRouter.show(kind, Math.round(node.audio.volume * 100), 100, node.audio.muted);
    }

    function settleSink() {
        prevSinkId = Q.osdSettledId(sink, sinkSettle.running);
    }
    function settleSource() {
        prevSourceId = Q.osdSettledId(source, sourceSettle.running);
    }

    onSinkChanged: {
        prevSinkId = null;
        sinkSettle.restart();
    }
    onSourceChanged: {
        prevSourceId = null;
        sourceSettle.restart();
    }

    // The settle delay or readiness, whichever comes later, records the id.
    Timer {
        id: sinkSettle
        interval: root.settleMs
        onTriggered: root.settleSink()
    }
    Timer {
        id: sourceSettle
        interval: root.settleMs
        onTriggered: root.settleSource()
    }

    PwObjectTracker {
        objects: [root.sink, root.source]
    }

    Connections {
        target: root.sink
        function onReadyChanged() { root.settleSink(); }
    }
    Connections {
        target: root.source
        function onReadyChanged() { root.settleSource(); }
    }
    Connections {
        target: root.sink ? root.sink.audio : null
        function onVolumesChanged() { root.level("volume", root.sink, root.prevSinkId); }
        function onMutedChanged() { root.level("volume", root.sink, root.prevSinkId); }
    }
    Connections {
        target: root.source ? root.source.audio : null
        function onVolumesChanged() { root.level("mic", root.source, root.prevSourceId); }
        function onMutedChanged() { root.level("mic", root.source, root.prevSourceId); }
    }
    Connections {
        target: Brightness
        function onChanged(percent) { OsdRouter.show("brightness", percent, 100, false); }
    }

    Component.onCompleted: {
        startedAt = Date.now();
        prevSinkId = null;
        prevSourceId = null;
        sinkSettle.restart();
        sourceSettle.restart();
    }
}
