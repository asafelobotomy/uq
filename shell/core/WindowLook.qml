pragma Singleton
import QtQuick

// How Hyprland draws windows right now (its border width, rounding, gaps and border colours),
// read through the bridge's WindowLook call. Anything the shell draws to stand for a window (the
// screen-edge indicator's frame) uses these, so it changes whenever the window look does: in
// Settings → Look, from a profile, a theme or a reload (EdgeIndicators passes on Hyprland's
// configreloaded; this file stays free of Quickshell so the core module loads under
// qmltestrunner). Until Hyprland answers, the theme's tokens stand in.
QtObject {
    id: root

    property bool known: false
    property int border: Theme.borderWidth
    property int rounding: Theme.radius
    property int gapsIn: Theme.gapIn
    // [top, right, bottom, left]
    property var gapsOut: [Theme.gapOut, Theme.gapOut, Theme.gapOut, Theme.gapOut]
    property color activeBorder: Theme.accent
    property color inactiveBorder: Theme.border

    readonly property int gapTop: gapsOut[0]
    readonly property int gapRight: gapsOut[1]
    readonly property int gapBottom: gapsOut[2]
    readonly property int gapLeft: gapsOut[3]
    // The outer corner radius of a window's frame: Hyprland rounds the border around the
    // window's own rounding.
    readonly property int frameRadius: rounding > 0 ? rounding + border : 0

    function apply(o) {
        if (!o || typeof o !== "object")
            return;
        function n(v, dflt) { return typeof v === "number" && v >= 0 ? Math.round(v) : dflt; }
        root.border = n(o.border, root.border);
        root.rounding = n(o.rounding, root.rounding);
        // Hyprland gives gaps_in as a box too; the gap between columns is its left value.
        root.gapsIn = Array.isArray(o.gaps_in) && o.gaps_in.length === 4 ? n(o.gaps_in[3], root.gapsIn)
            : n(o.gaps_in, root.gapsIn);
        if (Array.isArray(o.gaps_out) && o.gaps_out.length === 4)
            root.gapsOut = o.gaps_out.map(function (g) { return n(g, 0); });
        if (typeof o.active_border === "string" && /^#[0-9a-f]{8}$/.test(o.active_border))
            root.activeBorder = o.active_border;
        if (typeof o.inactive_border === "string" && /^#[0-9a-f]{8}$/.test(o.inactive_border))
            root.inactiveBorder = o.inactive_border;
        root.known = true;
    }

    function refresh() {
        Bridge.call("WindowLook", []).then(function (o) { root.apply(o); }).catch(function () {});
    }
    // Settings and profiles reach Hyprland a moment after uqd says they changed.
    property Timer later: Timer {
        interval: 400
        onTriggered: root.refresh()
    }
    function soon() { later.restart(); }

    Component.onCompleted: soon()
    property Connections bridge: Connections {
        target: Bridge
        function onSettingsChanged(s) { root.soon(); }
        function onThemeChanged() { root.soon(); }
        function onEvent(name, data) {
            if (name === "UqdUp" || name === "SectionChanged" || name === "Hello")
                root.soon();
        }
    }
}
