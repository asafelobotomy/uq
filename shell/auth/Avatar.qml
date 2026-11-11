import QtQuick
import "../core"

// A round avatar: the image at `avatar.path` (a file:// URL from auth.js avatarSource), else the
// initial in `avatar.initial`.
Rectangle {
    id: box

    // {path} or {initial}, from auth.js avatarSource.
    property var avatar: ({ initial: "?" })
    property string userName: ""
    property int size: 96

    readonly property bool hasImage: avatar && avatar.path !== undefined
    readonly property bool imageShown: hasImage && image.status === Image.Ready

    width: size
    height: size
    radius: width / 2
    color: Theme.surfaceAlt
    border.color: Theme.border
    border.width: Theme.borderWidth
    clip: true
    Accessible.role: Accessible.Graphic
    Accessible.name: "Avatar for " + userName

    Image {
        id: image
        anchors.fill: parent
        anchors.margins: Theme.borderWidth
        visible: box.hasImage && status === Image.Ready
        source: box.hasImage ? box.avatar.path : ""
        sourceSize.width: box.size * 2
        sourceSize.height: box.size * 2
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: false
    }
    UqText {
        anchors.centerIn: parent
        visible: box.avatar && box.avatar.initial !== undefined
        text: box.avatar && box.avatar.initial !== undefined ? box.avatar.initial : ""
        font.pixelSize: Math.round(box.size * 0.42)
        Accessible.ignored: true
    }
}
