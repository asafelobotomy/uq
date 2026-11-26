// The preview: the display's shape with the picture as the chosen fit draws it.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import ".."

ColumnLayout {
    // Passed in from WallpaperEntryEditor.qml.
    property var folderPath
    property var root

    spacing: Kirigami.Units.smallSpacing
    Layout.alignment: Qt.AlignTop
    Layout.preferredWidth: Kirigami.Units.gridUnit * 24
    Layout.fillWidth: root.columns === 1

    FitPicture {
        id: preview
        objectName: "preview"
        fit: root.shownFit
        backdrop: root.shownColor
        url: folderPath.text.trim() === "" ? root.pictureUrl : ""
        radius: Kirigami.Units.cornerRadius
        border.width: 1
        border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.2)
        Layout.fillWidth: true
        Layout.preferredHeight: width / Math.max(0.2, root.aspect)
        Accessible.role: Accessible.Graphic
        Accessible.name: "Wallpaper preview"
        Accessible.ignored: false

        // A slideshow, nothing at all, or a picture that can't be opened: say which, on a
        // plate in the window colours so it reads on any wallpaper colour.
        Rectangle {
            anchors.centerIn: parent
            visible: preview.url === "" || preview.failed
            color: Kirigami.Theme.backgroundColor
            opacity: 0.9
            radius: Kirigami.Units.cornerRadius
            implicitWidth: plate.implicitWidth + Kirigami.Units.largeSpacing * 2
            implicitHeight: plate.implicitHeight + Kirigami.Units.largeSpacing * 2
            ColumnLayout {
                id: plate
                anchors.centerIn: parent
                spacing: Kirigami.Units.smallSpacing
                Kirigami.Icon {
                    source: preview.failed ? "image-missing" : folderPath.text.trim() !== "" ? "folder-pictures" : "preferences-desktop-wallpaper"
                    implicitWidth: Kirigami.Units.iconSizes.large
                    implicitHeight: Kirigami.Units.iconSizes.large
                    Layout.alignment: Qt.AlignHCenter
                }
                UqLabel {
                    objectName: "previewCaption"
                    text: preview.failed ? "Can't open the picture" : folderPath.text.trim() !== "" ? "Slideshow" : "Colour only"
                    Layout.alignment: Qt.AlignHCenter
                }
            }
        }
    }
    RowLayout {
        visible: root.problem !== ""
        spacing: Kirigami.Units.smallSpacing
        Layout.fillWidth: true
        Kirigami.Icon {
            source: "dialog-warning"
            implicitWidth: Kirigami.Units.iconSizes.small
            implicitHeight: Kirigami.Units.iconSizes.small
            Layout.alignment: Qt.AlignTop
            Accessible.ignored: true
        }
        UqLabel {
            objectName: "problem"
            text: root.problem
            wrapMode: Text.Wrap
            color: Kirigami.Theme.negativeTextColor
            Layout.fillWidth: true
        }
    }
}
