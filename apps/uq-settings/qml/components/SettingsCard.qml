// A group of related settings on a SettingsTab: an icon and a short title over its rows, on a
// raised surface. `wide` makes it span both columns of the tab.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Rectangle {
    id: card

    property string title: ""
    property string iconName: ""
    /// A one-line explanation of the whole group, shown as a "?" by the title.
    property string help: ""
    /// Span both columns of the tab.
    property bool wide: false
    /// Controls beside the title (a Refresh button, an Add button).
    property alias actions: actionsRow.data
    default property alias content: body.data

    Layout.fillWidth: true
    Layout.alignment: Qt.AlignTop
    Layout.columnSpan: card.wide ? 2 : 1
    implicitWidth: Math.max(body.implicitWidth, head.implicitWidth) + Kirigami.Units.largeSpacing * 2
    implicitHeight: col.implicitHeight + Kirigami.Units.largeSpacing * 2

    radius: Kirigami.Units.cornerRadius * 2
    color: Kirigami.Theme.alternateBackgroundColor
    border.width: 1
    border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.08)

    Accessible.role: Accessible.Grouping
    Accessible.name: card.title

    ColumnLayout {
        id: col
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.smallSpacing

        RowLayout {
            id: head
            visible: card.title !== "" || actionsRow.children.length > 0
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true

            Kirigami.Icon {
                visible: card.iconName !== ""
                source: card.iconName
                implicitWidth: Kirigami.Units.iconSizes.smallMedium
                implicitHeight: Kirigami.Units.iconSizes.smallMedium
                Accessible.ignored: true
            }
            UqLabel {
                text: card.title
                font.bold: true
                elide: Text.ElideRight
                Accessible.role: Accessible.Heading
                Layout.fillWidth: true
            }
            HelpTip {
                tip: card.help
            }
            RowLayout {
                id: actionsRow
                spacing: Kirigami.Units.smallSpacing
            }
        }
        ColumnLayout {
            id: body
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true
        }
    }
}
