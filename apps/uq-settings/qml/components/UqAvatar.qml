// A user's picture in a circle, or their initial when there is none (or it can't be read).
// Kirigami 6.31 no longer ships Kirigami.Avatar in org.kde.kirigami, and a missing type made the
// whole Users page fail to load; this needs only Kirigami primitives that every KF6 has.
import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: avatar

    // A local file path ("" for none).
    property string path: ""
    // Whose picture it is; its first letter stands in for a missing picture.
    property string name: ""

    implicitWidth: Kirigami.Units.iconSizes.large
    implicitHeight: Kirigami.Units.iconSizes.large
    Accessible.role: Accessible.Graphic

    Kirigami.ShadowedRectangle {
        anchors.fill: parent
        radius: width / 2
        color: Kirigami.Theme.highlightColor
        visible: picture.status !== Image.Ready
        UqLabel {
            anchors.centerIn: parent
            text: avatar.name.length > 0 ? avatar.name.charAt(0).toUpperCase() : "?"
            color: Kirigami.Theme.highlightedTextColor
            font.bold: true
            font.pixelSize: Math.round(avatar.height * 0.45)
            Accessible.ignored: true
        }
    }

    Kirigami.ShadowedImage {
        id: picture
        anchors.fill: parent
        radius: width / 2
        source: avatar.path !== "" ? "file://" + avatar.path : ""
        fillMode: Image.PreserveAspectCrop
        sourceSize.width: width * Screen.devicePixelRatio
        sourceSize.height: height * Screen.devicePixelRatio
        asynchronous: true
        visible: status === Image.Ready
    }
}
