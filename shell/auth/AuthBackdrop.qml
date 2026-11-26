import QtQuick
import "../core"

// The background the login screen and the lock screen share: the login wallpaper (Settings → Login
// screen, copied to /var/lib/uq-greeter/wallpaper, world-readable) cropped to fill, else uQ's
// backdrop in the theme's colours. The picture is decoded only once the item has its size: a
// sourceSize of 0 decodes it at full size, and the real size then decodes it a second time.
Item {
    id: root

    // file:// URL of the picture; empty shows the backdrop.
    property string source: ""
    readonly property bool pictureShown: picture.visible

    UqBackdrop {
        anchors.fill: parent
        visible: !picture.visible
    }

    Image {
        id: picture
        objectName: "authWallpaper"
        anchors.fill: parent
        source: root.width > 0 && root.height > 0 ? root.source : ""
        sourceSize.width: root.width
        sourceSize.height: root.height
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: false
        visible: status === Image.Ready
    }
}
