import QtQuick
import "../core"

// Slice 1 placeholder: a static bell. The real notification centre arrives in a later slice.
Island {
    implicitWidth: 40
    UqText {
        anchors.centerIn: parent
        text: "\u{1F514}"
        color: Theme.textMuted
        font.pixelSize: Theme.fontSize + 3
    }
}
