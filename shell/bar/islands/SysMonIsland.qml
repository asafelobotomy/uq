import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets
import "../../core"
import "../../logic/ticker.js" as T
import "../../logic/shellprefs.js" as ShellPrefs
import ".."

// System monitor (`sysmon`). Icon: processor use. Small: a chip icon and processor use. Medium:
// and memory use. Large: and the processor's temperature where a sensor exists. A click shows
// every core and the memory in use. The graphics card (NVIDIA through nvidia-smi, AMD through
// sysfs) shows from Medium, after the memory, where there is one. The bridge reads /proc and /sys every 2 s, only while this
// island is placed.
Island {
    id: root

    readonly property var s: Bridge.sysmon
    readonly property bool has: s !== null && s !== undefined
    readonly property int iconSize: Theme.fontSize + 5
    // shell.bar.options.sysmon: which readings the island shows (the popup shows all of them).
    readonly property var opts: BarLayout.options.sysmon || ({})
    readonly property bool showCpu: opts.cpu !== false
    readonly property bool showMem: opts.mem !== false
    readonly property bool showGpu: opts.gpu !== false
    // shell.temp_unit: "c" or "f". Readings (and the 90 °C warning) stay Celsius.
    readonly property string unit: ShellPrefs.tempUnit(Settings.values.shell)
    readonly property bool showTemp: opts.temp !== false
    // Room for a second reading: Medium and wider, or Small when the processor is left out.
    readonly property bool roomy: size === "medium" || size === "large" || size === "custom"
        || (size === "small" && !showCpu)
    readonly property var gpu: has && s.gpu && typeof s.gpu === "object" ? s.gpu : null

    Accessible.role: Accessible.Button
    Accessible.name: has ? "Processor " + T.percent(s.cpu) + ", memory " + T.percent(s.mem)
        + (gpu ? ", graphics " + T.percent(gpu.busy) : "")
        + (s.temp_c !== null && s.temp_c !== undefined ? ", " + T.temp(s.temp_c, root.unit) : "")
        : "System monitor"

    RowLayout {
        anchors.centerIn: parent
        width: Math.min(implicitWidth, root.room)
        spacing: Theme.gapIn

        IconImage {
            visible: !root.isIcon
            implicitSize: root.iconSize
            Layout.preferredWidth: root.iconSize
            Layout.preferredHeight: root.iconSize
            source: Quickshell.iconPath("cpu", "utilities-system-monitor")
        }
        UqText {
            visible: root.showCpu || (!root.showMem && !root.showGpu)
            text: root.has ? T.percent(root.s.cpu) : "–"
            baseFamily: Theme.fontMono
            Layout.minimumWidth: implicitWidth
        }
        UqText {
            visible: root.showMem && root.roomy
            text: "RAM " + (root.has ? T.percent(root.s.mem) : "–")
            baseFamily: Theme.fontMono
            color: root.showCpu ? Theme.textMuted : Theme.text
            Layout.minimumWidth: implicitWidth
        }
        UqText {
            visible: root.showGpu && root.gpu !== null && root.roomy
            text: "GPU " + (root.gpu ? T.percent(root.gpu.busy) : "")
            baseFamily: Theme.fontMono
            color: Theme.textMuted
            Layout.minimumWidth: implicitWidth
        }
        UqText {
            visible: root.showTemp && (root.size === "large" || root.size === "custom") && root.has && T.temp(root.s.temp_c, root.unit) !== ""
            text: root.has ? T.temp(root.s.temp_c, root.unit) : ""
            baseFamily: Theme.fontMono
            color: root.has && root.s.temp_c >= 90 ? Theme.warning : Theme.textMuted
            Layout.minimumWidth: implicitWidth
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: popup.open && popup.anchorItem === root ? popup.hide() : popup.show(root, details)
    }

    LazyPopup {
        id: popup
    }

    Component {
        id: details
        ColumnLayout {
            spacing: Theme.gapIn
            width: Theme.fontSize * 24

            UqText {
                text: "Processor " + (root.has ? T.percent(root.s.cpu) : "–")
                    + (root.has && T.temp(root.s.temp_c, root.unit) !== "" ? " · " + T.temp(root.s.temp_c, root.unit) : "")
                baseBold: true
            }
            GridLayout {
                columns: 2
                columnSpacing: Theme.gapOut * 2
                rowSpacing: 2
                Layout.fillWidth: true
                Repeater {
                    model: root.has && Array.isArray(root.s.cores) ? root.s.cores.slice(0, 64) : []
                    delegate: RowLayout {
                        required property var modelData
                        required property int index
                        spacing: Theme.gapIn
                        Layout.fillWidth: true
                        UqText {
                            text: String(index + 1)
                            color: Theme.textMuted
                            baseFamily: Theme.fontMono
                            Layout.preferredWidth: Theme.fontSize * 2
                        }
                        // A bar for the core's load; the number beside it says the same.
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: Math.max(2, Math.round(Theme.fontSize / 2))
                            radius: height / 2
                            color: Theme.surfaceAlt
                            Rectangle {
                                width: parent.width * Math.max(0, Math.min(100, Number(modelData) || 0)) / 100
                                height: parent.height
                                radius: parent.radius
                                color: Theme.accent
                            }
                        }
                        UqText {
                            text: T.percent(modelData)
                            baseFamily: Theme.fontMono
                            Layout.preferredWidth: Theme.fontSize * 3
                            horizontalAlignment: Text.AlignRight
                        }
                    }
                }
            }
            UqText {
                text: "Memory " + (root.has ? T.percent(root.s.mem) + " · " + T.bytes(root.s.mem_used)
                    + " of " + T.bytes(root.s.mem_total) : "–")
                baseBold: true
            }
            UqText {
                visible: root.gpu !== null
                text: root.gpu ? "Graphics card " + T.percent(root.gpu.busy)
                    + (T.temp(root.gpu.temp_c, root.unit) !== "" ? " · " + T.temp(root.gpu.temp_c, root.unit) : "") : ""
                baseBold: true
            }
            UqText {
                visible: root.gpu !== null
                text: root.gpu ? String(root.gpu.name).slice(0, 48)
                    + (root.gpu.vram_total > 0 ? " · video memory " + T.bytes(root.gpu.vram_used)
                    + " of " + T.bytes(root.gpu.vram_total) : "") : ""
                color: Theme.textMuted
                Layout.fillWidth: true
                wrapMode: Text.Wrap
            }
            MenuRow {
                id: openMonitor
                Layout.fillWidth: true
                text: "Open a system monitor"
                onActivated: {
                    Bridge.call("Launch", [{ desktop_id: "org.gnome.SystemMonitor.desktop" }]).catch(function () {
                        Bridge.call("Launch", [{ desktop_id: "org.kde.plasma-systemmonitor.desktop" }]).catch(function () {
                            Bridge.call("Launch", [{ desktop_id: "btop.desktop" }]).catch(function () {});
                        });
                    });
                    popup.hide();
                }
                Component.onCompleted: forceActiveFocus()
            }
        }
    }
}
