import QtQuick
import QtQuick.Layouts
import "../core"
import "../brightness"

// Quick settings: the screen brightness slider. Hidden when brightnessctl finds no backlight.
ColumnLayout {
    id: root

    // Set by QuickSettings: the window is open. Closing it drops a pending write.
    property bool active: false
    property int pendingBrightness: -1

    visible: Brightness.available
    spacing: Theme.gapIn * 2

    function wipe() {
        pendingBrightness = -1;
        brightRetry.stop();
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

    Timer {
        id: brightRetry
        interval: 60
        onTriggered: root.flushBrightness()
    }

    UqText {
        text: "Brightness"
        color: Theme.textMuted
        font.bold: true
    }

    RowLayout {
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
