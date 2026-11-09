import QtQuick
import Quickshell
import Quickshell.Services.Pipewire
import "../brightness"
import "../logic/quick.js" as Q

// Window-free OSD source (safe to create headless). Volume and mute changes on the default sink
// and source go to OsdRouter, gated by osdShouldShow: nothing in the first 2 s, and nothing for a
// default-device switch (the new node's first change only records its id). Brightness OSDs come
// from Brightness.changed.
Scope {
    id: root

    property real startedAt: NaN
    property var prevSinkId: null
    property var prevSourceId: null
    readonly property PwNode sink: Pipewire.defaultAudioSink
    readonly property PwNode source: Pipewire.defaultAudioSource

    // One level change on `node`; `prev` names the property holding that kind's last node id.
    function level(kind, node, prev) {
        if (!node || !node.audio)
            return;
        var id = node.id;
        if (Q.osdShouldShow(startedAt, Date.now(), root[prev], id))
            OsdRouter.show(kind, Math.round(node.audio.volume * 100), 100, node.audio.muted);
        root[prev] = id;
    }

    PwObjectTracker {
        objects: [root.sink, root.source]
    }

    Connections {
        target: root.sink ? root.sink.audio : null
        function onVolumesChanged() { root.level("volume", root.sink, "prevSinkId"); }
        function onMutedChanged() { root.level("volume", root.sink, "prevSinkId"); }
    }
    Connections {
        target: root.source ? root.source.audio : null
        function onVolumesChanged() { root.level("mic", root.source, "prevSourceId"); }
        function onMutedChanged() { root.level("mic", root.source, "prevSourceId"); }
    }
    Connections {
        target: Brightness
        function onChanged(percent) { OsdRouter.show("brightness", percent, 100, false); }
    }

    Component.onCompleted: startedAt = Date.now()
}
