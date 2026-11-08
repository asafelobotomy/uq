pragma Singleton
import QtQuick

// Mirrors uqd settings (GetSettings) plus launch counts. Refreshes on UqdUp / SettingsChanged.
QtObject {
    id: root

    property var values: ({})
    property var launchCounts: ({})

    function refresh() {
        Bridge.call("GetSettings", []).then(function (s) {
            if (s && typeof s === "object")
                root.values = s;
        }).catch(function () {});
        Bridge.call("GetLaunchCounts", []).then(function (c) {
            if (c && typeof c === "object")
                root.launchCounts = c;
        }).catch(function () {});
    }

    property Connections conn: Connections {
        target: Bridge
        function onSettingsChanged(s) {
            if (s && typeof s === "object")
                root.values = s;
        }
        function onEvent(name, data) {
            if (name === "UqdUp")
                root.refresh();
            else if (name === "LaunchCountsChanged" && data && typeof data === "object")
                root.launchCounts = data;
            else if (name === "Hello" && data && data.uqd_up)
                root.refresh();
        }
    }
}
