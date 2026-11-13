import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import "../../core"
import ".."

// Keyboard layout (`keyboard`): the active layout's short name (`US`, `DE`); a click switches
// to the next. Hidden with fewer than two layouts (Settings → Input). Hyprland says when the
// layout changes; the index comes from `hyprctl devices -j` (the main keyboard).
Island {
    id: root

    readonly property var layouts: {
        var i = Settings.values.input;
        var l = i && i.keyboard && Array.isArray(i.keyboard.layouts) ? i.keyboard.layouts : [];
        return l.filter(function (x) { return x && typeof x.layout === "string"; });
    }
    property int active: 0
    property string keymap: ""
    readonly property string label: layouts.length > 0
        ? String(layouts[Math.max(0, Math.min(active, layouts.length - 1))].layout).toUpperCase().slice(0, 8) : ""

    shown: layouts.length >= 2
    implicitWidth: layoutText.implicitWidth + Theme.gapIn * 4
    Accessible.role: Accessible.Button
    Accessible.name: "Keyboard layout: " + (keymap !== "" ? keymap : label) + ". Switch layout"

    UqText {
        id: layoutText
        anchors.centerIn: parent
        text: root.label
        font.family: Theme.fontMono
    }

    MouseArea {
        anchors.fill: parent
        onClicked: if (!switcher.running) switcher.running = true
    }

    Process {
        id: switcher
        command: ["hyprctl", "switchxkblayout", "all", "next"]
    }

    Process {
        id: devices
        command: ["hyprctl", "devices", "-j"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    var d = JSON.parse(text);
                    var ks = Array.isArray(d.keyboards) ? d.keyboards : [];
                    var k = null;
                    for (var i = 0; i < ks.length; i++)
                        if (ks[i] && ks[i].main === true)
                            k = ks[i];
                    if (k === null && ks.length > 0)
                        k = ks[0];
                    if (k !== null) {
                        root.active = typeof k.active_layout_index === "number" ? k.active_layout_index : 0;
                        root.keymap = typeof k.active_keymap === "string" ? k.active_keymap.slice(0, 60) : "";
                    }
                } catch (e) {
                }
            }
        }
    }

    function refresh() {
        if (shown && !devices.running)
            devices.running = true;
    }
    onShownChanged: refresh()
    onLayoutsChanged: refresh()
    Component.onCompleted: refresh()

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "activelayout")
                root.refresh();
        }
    }
}
