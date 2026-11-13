import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets
import Quickshell.Hyprland
import Quickshell.Io
import "../../core"
import ".."

// Keyboard layout (`keyboard`). Icon: the active layout's short name (`US`, `DE`, at most 3
// letters). Small: a keyboard icon and the name. Medium and Large: the icon and the layout's
// full name (`English (US)`). A click switches to the next layout. Hidden with fewer than two layouts (Settings → Input). Hyprland says when the
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
        ? String(layouts[Math.max(0, Math.min(active, layouts.length - 1))].layout).toUpperCase().slice(0, isIcon ? 3 : 8) : ""

    shown: layouts.length >= 2
    Accessible.role: Accessible.Button
    Accessible.name: "Keyboard layout: " + (keymap !== "" ? keymap : label) + ". Switch layout"

    RowLayout {
        anchors.centerIn: parent
        width: Math.min(implicitWidth, root.room)
        spacing: Theme.gapIn
        IconImage {
            visible: !root.isIcon
            implicitSize: Theme.fontSize + 5
            Layout.preferredWidth: implicitSize
            Layout.preferredHeight: implicitSize
            source: Quickshell.iconPath("input-keyboard", true)
        }
        UqText {
            id: layoutText
            text: (root.size === "medium" || root.size === "large") && root.keymap !== "" ? root.keymap : root.label
            font.family: root.isIcon || root.size === "small" ? Theme.fontMono : Theme.fontUi
            Layout.fillWidth: true
        }
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
