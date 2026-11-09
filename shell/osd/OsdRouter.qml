pragma Singleton
import QtQuick
import "../core"
import "../notif"
import "../logic/quick.js" as Q

// One entry point for every OSD (spec §2.6): noti-fi's OSD when it is available, otherwise the
// fallback Osd.qml, which binds to the state below. A failed noti-fi request falls back too.
QtObject {
    id: root

    // --- fallback state, bound by Osd.qml ---
    property string kind: ""
    property real value: 0
    property real max: 100
    property bool muted: false
    // Incremented on each fallback show; Osd.qml restarts its hide timer on change.
    property int serial: 0

    function show(kind, value, max, muted) {
        if (!Q.osdAllowed(Surfaces.open))
            return;
        if (!Notifi.available) {
            fallback(kind, value, max, muted);
            return;
        }
        Notifi.osd(kind, value, max, muted).catch(function () {
            root.fallback(kind, value, max, muted);
        });
    }

    function fallback(kind, value, max, muted) {
        root.kind = String(kind);
        root.value = Number(value) || 0;
        root.max = Number(max) > 0 ? Number(max) : 100;
        root.muted = !!muted;
        serial++;
    }
}
