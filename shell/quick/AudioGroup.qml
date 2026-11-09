pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell.Services.Pipewire
import "../core"
import "../brightness"
import "../logic/quick.js" as Q
import "../logic/quickui.js" as U

// Quick settings: output volume, mute and device; mic mute and input volume; brightness.
// Pipewire node labels (description / nickname / name) are untrusted: UqText only, elided.
// With no default sink (unplugged headphones, Pipewire not up) it shows "No output".
ColumnLayout {
    id: root

    // Set by QuickSettings: the window is open. Nodes are only tracked while it is.
    property bool active: false
    property bool showDevices: false
    property int pendingBrightness: -1

    readonly property var sink: Pipewire.defaultAudioSink
    readonly property var source: Pipewire.defaultAudioSource
    readonly property var sinkAudio: sink && sink.audio ? sink.audio : null
    readonly property var sourceAudio: source && source.audio ? source.audio : null
    readonly property var sinks: U.sinkNodes(Pipewire.nodes.values)
    readonly property var tracked: {
        if (!active)
            return [];
        var out = sinks.slice();
        if (sink && out.indexOf(sink) < 0)
            out.push(sink);
        if (source)
            out.push(source);
        return out;
    }

    spacing: Theme.gapIn * 2

    function wipe() {
        showDevices = false;
        pendingBrightness = -1;
        brightRetry.stop();
    }

    // A slider move: raising from 0 while muted unmutes (quickui.volumeSet).
    function setVolume(audio, v) {
        if (!audio)
            return;
        var r = U.volumeSet(audio.volume, v, audio.muted);
        if (r === null)
            return;
        audio.volume = r.volume;
        if (r.unmute)
            audio.muted = false;
    }
    function stepVolume(audio, dir) {
        if (audio)
            setVolume(audio, Q.volumeStep(audio.volume, dir));
    }

    // One brightnessctl at a time: while one runs, keep the latest value and retry shortly, so a
    // drag lands on where the handle stopped without queueing a process per move.
    function setBrightness(fraction) {
        pendingBrightness = Math.round(fraction * 100);
        flushBrightness();
    }
    function flushBrightness() {
        if (pendingBrightness < 0)
            return;
        if (Brightness.set(pendingBrightness))
            pendingBrightness = -1;
        else if (Brightness.available)
            brightRetry.restart();
        else
            pendingBrightness = -1;
    }

    onActiveChanged: if (!active) wipe()

    PwObjectTracker {
        objects: root.tracked
    }

    Timer {
        id: brightRetry
        interval: 60
        onTriggered: root.flushBrightness()
    }

    // --- shared controls ---
    component QsSlider: Item {
        id: s
        property real value: 0          // 0..1
        property bool dim: false
        property bool dragging: mouse.pressed
        property real dragValue: 0
        readonly property real shown: Math.max(0, Math.min(1, dragging ? dragValue : value))
        signal moved(real v)
        signal stepped(int dir)

        implicitHeight: 24
        activeFocusOnTab: true
        Accessible.role: Accessible.Slider
        Accessible.onIncreaseAction: s.stepped(1)
        Accessible.onDecreaseAction: s.stepped(-1)

        function pick(x) {
            dragValue = Math.max(0, Math.min(1, x / Math.max(1, width)));
            moved(dragValue);
        }

        Keys.onPressed: function (event) {
            if (event.key === Qt.Key_Right || event.key === Qt.Key_Up)
                s.stepped(1);
            else if (event.key === Qt.Key_Left || event.key === Qt.Key_Down)
                s.stepped(-1);
            else
                return;
            event.accepted = true;
        }

        Rectangle {
            id: track
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width
            height: 6
            radius: 3
            color: Theme.overlay
            Rectangle {
                width: track.width * s.shown
                height: parent.height
                radius: parent.radius
                color: s.dim ? Theme.textMuted : Theme.accent
            }
        }
        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            x: s.shown * (s.width - width)
            width: 14
            height: 14
            radius: 7
            color: Theme.text
            border.color: s.activeFocus ? Theme.accent : Theme.border
            border.width: s.activeFocus ? 2 : 1
        }
        MouseArea {
            id: mouse
            anchors.fill: parent
            onPressed: function (m) { s.forceActiveFocus(); s.pick(m.x); }
            onPositionChanged: function (m) { if (pressed) s.pick(m.x); }
            onWheel: function (w) {
                if (w.angleDelta.y !== 0)
                    s.stepped(w.angleDelta.y > 0 ? 1 : -1);
            }
        }
    }

    component IconButton: Rectangle {
        id: b
        property string glyph: ""
        property bool on: false
        signal clicked()

        implicitWidth: 28
        implicitHeight: 28
        radius: Theme.radius - 2
        color: on ? Theme.accent : (bMouse.containsMouse ? Theme.overlay : Theme.surfaceAlt)
        activeFocusOnTab: true
        border.color: Theme.accent
        border.width: activeFocus ? 1 : 0
        Accessible.role: Accessible.Button
        Accessible.onPressAction: b.clicked()
        Keys.onSpacePressed: b.clicked()
        Keys.onReturnPressed: b.clicked()

        UqText {
            anchors.centerIn: parent
            text: b.glyph
            color: b.on ? Theme.accentText : Theme.text
            font.pixelSize: Theme.fontSize + 2
        }
        MouseArea {
            id: bMouse
            anchors.fill: parent
            hoverEnabled: true
            onClicked: b.clicked()
        }
    }

    UqText {
        text: "Audio"
        color: Theme.textMuted
        font.bold: true
    }

    // --- output ---
    UqText {
        // The device header below says "Choose output" when sinks exist but none is default.
        visible: root.sinkAudio === null && root.sinks.length === 0
        Layout.fillWidth: true
        text: "No output"
        color: Theme.textMuted
    }
    RowLayout {
        visible: root.sinkAudio !== null
        Layout.fillWidth: true
        spacing: Theme.gapIn * 2

        IconButton {
            readonly property bool muted: root.sinkAudio ? root.sinkAudio.muted : false
            glyph: muted ? "🔇" : "🔊"
            on: muted
            Accessible.name: muted ? "Unmute output" : "Mute output"
            onClicked: if (root.sinkAudio) root.sinkAudio.muted = !root.sinkAudio.muted
        }
        QsSlider {
            Layout.fillWidth: true
            value: root.sinkAudio ? root.sinkAudio.volume : 0
            dim: root.sinkAudio ? root.sinkAudio.muted : false
            Accessible.name: "Output volume"
            onMoved: function (v) { root.setVolume(root.sinkAudio, v); }
            onStepped: function (dir) { root.stepVolume(root.sinkAudio, dir); }
        }
        UqText {
            Layout.preferredWidth: 34
            horizontalAlignment: Text.AlignRight
            text: root.sinkAudio ? Math.round(root.sinkAudio.volume * 100) + "%" : ""
            color: Theme.textMuted
        }
    }

    // Output device: the current sink, expanding to the list of hardware sinks.
    Rectangle {
        id: deviceHeader
        visible: root.sinks.length > 0
        Layout.fillWidth: true
        implicitHeight: 28
        radius: Theme.radius - 2
        color: headerMouse.containsMouse ? Theme.overlay : "transparent"
        activeFocusOnTab: true
        border.color: Theme.accent
        border.width: activeFocus ? 1 : 0
        Accessible.role: Accessible.Button
        Accessible.name: (root.showDevices ? "Hide output devices, current: " : "Choose output device, current: ")
            + (root.sink ? U.nodeLabel(root.sink) : "none")
        Accessible.onPressAction: root.showDevices = !root.showDevices
        Keys.onSpacePressed: root.showDevices = !root.showDevices
        Keys.onReturnPressed: root.showDevices = !root.showDevices

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Theme.gapIn * 2
            anchors.rightMargin: Theme.gapIn * 2
            spacing: Theme.gapIn
            UqText {
                Layout.fillWidth: true
                maximumLineCount: 1
                text: root.sink ? U.nodeLabel(root.sink) : "Choose output"
            }
            UqText {
                text: root.showDevices ? "▴" : "▾"
                color: Theme.textMuted
            }
        }
        MouseArea {
            id: headerMouse
            anchors.fill: parent
            hoverEnabled: true
            onClicked: root.showDevices = !root.showDevices
        }
    }
    Repeater {
        model: root.showDevices ? root.sinks : []
        delegate: Rectangle {
            id: devRow
            required property var modelData
            readonly property bool current: root.sink === modelData
            readonly property string label: U.nodeLabel(modelData)
            Layout.fillWidth: true
            implicitHeight: 26
            radius: Theme.radius - 2
            color: current ? Theme.overlay : (devMouse.containsMouse ? Theme.surfaceAlt : "transparent")
            activeFocusOnTab: true
            border.color: Theme.accent
            border.width: activeFocus ? 1 : 0
            Accessible.role: Accessible.RadioButton
            Accessible.checkable: true
            Accessible.checked: current
            Accessible.name: "Output device " + label
            Accessible.onPressAction: devRow.choose()
            Keys.onSpacePressed: devRow.choose()
            Keys.onReturnPressed: devRow.choose()

            function choose() {
                Pipewire.preferredDefaultAudioSink = devRow.modelData;
            }

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: Theme.gapIn * 4
                anchors.rightMargin: Theme.gapIn * 2
                spacing: Theme.gapIn
                UqText {
                    Layout.preferredWidth: 12
                    text: devRow.current ? "●" : ""
                    color: Theme.accent
                }
                UqText {
                    Layout.fillWidth: true
                    maximumLineCount: 1
                    text: devRow.label
                    color: devRow.current ? Theme.text : Theme.textMuted
                }
            }
            MouseArea {
                id: devMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: devRow.choose()
            }
        }
    }

    // --- input ---
    RowLayout {
        visible: root.sourceAudio !== null
        Layout.fillWidth: true
        spacing: Theme.gapIn * 2

        IconButton {
            readonly property bool muted: root.sourceAudio ? root.sourceAudio.muted : false
            glyph: muted ? "🚫" : "🎤"
            on: muted
            Accessible.name: muted ? "Unmute microphone" : "Mute microphone"
            onClicked: if (root.sourceAudio) root.sourceAudio.muted = !root.sourceAudio.muted
        }
        QsSlider {
            Layout.fillWidth: true
            value: root.sourceAudio ? root.sourceAudio.volume : 0
            dim: root.sourceAudio ? root.sourceAudio.muted : false
            Accessible.name: "Input volume"
            onMoved: function (v) { root.setVolume(root.sourceAudio, v); }
            onStepped: function (dir) { root.stepVolume(root.sourceAudio, dir); }
        }
        UqText {
            Layout.preferredWidth: 34
            horizontalAlignment: Text.AlignRight
            text: root.sourceAudio ? Math.round(root.sourceAudio.volume * 100) + "%" : ""
            color: Theme.textMuted
        }
    }

    // --- brightness ---
    RowLayout {
        visible: Brightness.available
        Layout.fillWidth: true
        spacing: Theme.gapIn * 2

        UqText {
            Layout.preferredWidth: 28
            horizontalAlignment: Text.AlignHCenter
            text: "☀"
            font.pixelSize: Theme.fontSize + 2
            Accessible.ignored: true
        }
        QsSlider {
            Layout.fillWidth: true
            value: Brightness.percent / 100
            Accessible.name: "Brightness"
            onMoved: function (v) { root.setBrightness(v); }
            onStepped: function (dir) { Brightness.step(dir); }
        }
        UqText {
            Layout.preferredWidth: 34
            horizontalAlignment: Text.AlignRight
            text: Brightness.percent + "%"
            color: Theme.textMuted
        }
    }
}
