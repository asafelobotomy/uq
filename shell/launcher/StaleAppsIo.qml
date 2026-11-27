import QtQuick
import Quickshell
import "../core"

// Sends Quickshell's desktop entry ids to StaleApps when the list changes (installs and
// removals rescan it), a moment after the last change. Instantiate once (shell.qml).
Item {
    Timer {
        id: settle
        interval: 400
        onTriggered: {
            var v = DesktopEntries.applications.values;
            var ids = [];
            for (var i = 0; i < v.length; i++)
                if (v[i] && v[i].id)
                    ids.push(String(v[i].id));
            StaleApps.refresh(ids);
        }
    }
    Connections {
        target: DesktopEntries.applications
        function onValuesChanged() { settle.restart(); }
    }
    Component.onCompleted: settle.restart()
}
