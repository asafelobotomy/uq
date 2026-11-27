pragma Singleton
import QtQuick

// The desktop entries the app menu and look=see>do_ leave out (the bridge's StaleApps): their
// program is gone (a deleted AppImage, a launcher left behind by a removed package) or the file
// is a symlinked alias of another listed entry. `hidden` maps those ids to true. StaleAppsIo
// (launcher/) sends the ids whenever Quickshell's list of entries changes; this file stays free
// of Quickshell so the core module loads under qmltestrunner. Until the bridge answers, or when
// it can't, nothing is hidden.
QtObject {
    id: root

    property var hidden: ({})
    property var ids: []

    function refresh(list) {
        root.ids = list || [];
        if (root.ids.length === 0) {
            root.hidden = {};
            return;
        }
        Bridge.call("StaleApps", [root.ids]).then(function (r) {
            var m = {};
            if (r && r.length !== undefined)
                for (var i = 0; i < r.length; i++)
                    if (typeof r[i] === "string")
                        m[r[i]] = true;
            root.hidden = m;
        }).catch(function () {});
    }

    property Connections conn: Connections {
        target: Bridge
        function onEvent(name, data) {
            // A restarted bridge: ask again.
            if (name === "Hello" && root.ids.length > 0)
                root.refresh(root.ids);
        }
    }
}
