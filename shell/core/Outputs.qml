pragma Singleton
import QtQuick
import "../logic/outputs.js" as O

// The display roles uqd keeps (the main display and which screens get a bar), read through the
// bridge's ListOutputs. Pure QML, no Quickshell imports, so it loads under qmltestrunner; the
// screens come in as arguments. `roles` stays null until the first answer (and while uqd is
// down it keeps the last one), which means "every screen, no main display".
QtObject {
    id: root

    // {primary, main, bar} as uqd sends it, or null.
    property var roles: null

    // Only the newest answer counts, so a slow reply can't overwrite a newer one.
    property int _serial: 0

    function refresh() {
        var mine = ++root._serial;
        Bridge.call("ListOutputs", []).then(function (out) {
            if (mine !== root._serial)
                return;
            var r = out && typeof out === "object" ? out.roles : null;
            root.roles = r && typeof r === "object" ? r : null;
        }).catch(function () {});
    }

    function primaryScreen(screens) {
        return O.primaryScreen(screens, root.roles);
    }

    function barScreens(screens) {
        return O.barScreens(screens, root.roles);
    }

    property Connections bridgeConn: Connections {
        target: Bridge
        function onEvent(name, data) {
            if (name === "UqdUp" || name === "MonitorsChanged"
                    || (name === "Hello" && data && data.uqd_up)
                    || (name === "SectionChanged" && data && data.domain === "monitors"))
                root.refresh();
        }
    }

    Component.onCompleted: {
        if (Bridge.uqdUp)
            root.refresh();
    }
}
