import QtQuick
import Qt.labs.platform

// A tray icon (a StatusNotifierItem) with a two-row menu, for scripts/popups-live.sh: run with
// `qml6 tray-app.qml -- NAME ICON_FILE`. It prints "TRAYACT NAME" when it is activated and
// "TRAYMENU NAME" when its menu's first row is chosen. No window.
QtObject {
    readonly property var args: Qt.application.arguments
    readonly property string name: args[args.length - 2]

    property SystemTrayIcon icon: SystemTrayIcon {
        visible: true
        icon.source: "file://" + args[args.length - 1]
        tooltip: "Tray " + name
        menu: Menu {
            MenuItem {
                text: "Open " + name
                onTriggered: console.log("TRAYMENU " + name)
            }
            MenuItem {
                text: "Quit"
                onTriggered: Qt.quit()
            }
        }
        onActivated: function (reason) {
            console.log("TRAYACT " + name + " " + reason);
        }
    }
}
