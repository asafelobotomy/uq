// The picture drawn as a fit would show it on a screen of this shape: cropped to fill, whole
// inside, small in the middle, repeated, or (span) cropped like fill.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import ".."

Rectangle {
    id: pic
    property string fit: "fill"
    property string url: ""
    /// The colour behind the picture.
    property color backdrop: "transparent"
    /// The picture couldn't be opened (a moved or unreadable file).
    readonly property bool failed: one.status === Image.Error
    color: pic.backdrop
    clip: true
    Accessible.ignored: true

    Image {
        id: one
        visible: pic.fit !== "tile" && pic.url !== ""
        source: pic.url
        asynchronous: true
        sourceSize.width: Kirigami.Units.gridUnit * 40
        fillMode: pic.fit === "fit" || pic.fit === "center" ? Image.PreserveAspectFit : Image.PreserveAspectCrop
        anchors.centerIn: parent
        width: pic.fit === "center" ? parent.width * 0.5 : parent.width
        height: pic.fit === "center" ? parent.height * 0.5 : parent.height
    }
    Grid {
        visible: pic.fit === "tile" && pic.url !== ""
        columns: 3
        Repeater {
            model: pic.fit === "tile" && pic.url !== "" ? 9 : 0
            Image {
                source: pic.url
                asynchronous: true
                sourceSize.width: Kirigami.Units.gridUnit * 40
                fillMode: Image.PreserveAspectCrop
                width: pic.width / 3
                height: pic.height / 3
            }
        }
    }
}
