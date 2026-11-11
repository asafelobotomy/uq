import QtQuick

Item {
    property var ctx
    Loader {
        anchors.centerIn: parent
        sourceComponent: parent.ctx ? parent.ctx.UqText : null
        onLoaded: item.text = "G"
    }
}
