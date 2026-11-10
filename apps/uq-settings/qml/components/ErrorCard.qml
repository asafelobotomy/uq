// A card for something that failed: a title and the error text, both plain text.
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Rectangle {
    id: card

    property string title
    property string detail

    implicitWidth: Kirigami.Units.gridUnit * 24
    implicitHeight: column.implicitHeight + Kirigami.Units.largeSpacing * 2
    radius: Kirigami.Units.cornerRadius
    color: Qt.rgba(Kirigami.Theme.negativeTextColor.r, Kirigami.Theme.negativeTextColor.g,
                   Kirigami.Theme.negativeTextColor.b, 0.12)
    border.width: 1
    border.color: Kirigami.Theme.negativeTextColor

    Accessible.role: Accessible.AlertMessage
    Accessible.name: title
    Accessible.description: detail

    ColumnLayout {
        id: column
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.smallSpacing

        UqLabel {
            objectName: "errorTitle"
            text: card.title
            font.bold: true
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "errorDetail"
            text: card.detail
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
    }
}
