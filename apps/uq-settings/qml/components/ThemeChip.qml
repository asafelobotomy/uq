// One theme on the Appearance page: a tile that switches to it, showing a small window drawn in
// the theme's colours (`theme.swatches`, when known) over its name, a "Yours" tag for the user's
// own themes and a line for one whose files were removed. The user's themes also have a menu:
// Edit, Export and Delete. Names come from theme files anyone can write, so they are shown
// through UqLabel (plain text), never as a stock control's `text:`.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

Item {
    id: chip

    /// `{id, name, user, active, missing, swatches?}`; `swatches` is `[background, panels, text,
    /// accent, …]` as colour strings, when the theme's colours are known.
    property var theme: ({})
    readonly property string themeId: String(theme.id ?? "")
    readonly property string themeName: String(theme.name ?? "")
    readonly property bool current: theme.active === true
    readonly property bool isUser: theme.user === true
    readonly property var swatches: theme.swatches && theme.swatches.length > 0 ? Array.from(theme.swatches).map(String) : []
    readonly property bool hasColours: swatches.length >= 4

    /// The theme was chosen: switch to it.
    signal chosen(string id)
    /// **Edit…**, **Export…** and **Delete…** (the user's themes only).
    signal edit(string id)
    signal exportTheme(string id)
    signal remove(string id)

    implicitWidth: Kirigami.Units.gridUnit * 9
    implicitHeight: pick.implicitHeight
    Accessible.role: Accessible.Grouping
    Accessible.name: "Theme " + chip.themeName

    QQC2.Button {
        id: pick
        objectName: "themeButton"
        anchors.fill: parent
        checkable: true
        checked: chip.current
        Accessible.name: "Theme " + chip.themeName
        QQC2.ToolTip.visible: hovered && nameLabel.truncated
        QQC2.ToolTip.text: chip.themeName
        contentItem: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            // A small window in the theme's colours: background, a panel bar, text lines and an
            // accent button. Without known colours, a neutral tile with a palette icon.
            Rectangle {
                objectName: "preview"
                radius: Kirigami.Units.cornerRadius
                color: chip.hasColours ? chip.swatches[0] : Kirigami.Theme.backgroundColor
                border.width: 1
                border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.15)
                clip: true
                Layout.fillWidth: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 4
                Accessible.ignored: true

                Rectangle {
                    visible: chip.hasColours
                    color: chip.hasColours ? chip.swatches[1] : "transparent"
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 1
                    height: parent.height * 0.22
                }
                Column {
                    visible: chip.hasColours
                    spacing: Kirigami.Units.smallSpacing
                    x: Kirigami.Units.smallSpacing * 2
                    y: parent.height * 0.22 + Kirigami.Units.smallSpacing * 2
                    Repeater {
                        model: [0.55, 0.4]
                        Rectangle {
                            required property real modelData
                            width: chip.width * modelData
                            height: Kirigami.Units.smallSpacing
                            radius: height / 2
                            color: chip.hasColours ? chip.swatches[2] : "transparent"
                            opacity: 0.8
                        }
                    }
                }
                Rectangle {
                    visible: chip.hasColours
                    color: chip.hasColours ? chip.swatches[3] : "transparent"
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.margins: Kirigami.Units.smallSpacing * 2
                    width: parent.width * 0.3
                    height: Kirigami.Units.smallSpacing * 3
                    radius: Kirigami.Units.cornerRadius
                }
                Row {
                    visible: chip.swatches.length > 4
                    spacing: 2
                    anchors.left: parent.left
                    anchors.bottom: parent.bottom
                    anchors.margins: Kirigami.Units.smallSpacing * 2
                    Repeater {
                        model: chip.swatches.slice(4)
                        Rectangle {
                            required property var modelData
                            width: Kirigami.Units.smallSpacing * 2
                            height: width
                            radius: width / 2
                            color: modelData
                        }
                    }
                }
                Kirigami.Icon {
                    visible: !chip.hasColours
                    source: "color-management"
                    anchors.centerIn: parent
                    implicitWidth: Kirigami.Units.iconSizes.medium
                    implicitHeight: Kirigami.Units.iconSizes.medium
                    opacity: 0.5
                }
                Kirigami.Icon {
                    visible: chip.current
                    source: "emblem-checked"
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.margins: Kirigami.Units.smallSpacing
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: Kirigami.Units.iconSizes.small
                }
            }
            RowLayout {
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.smallSpacing
                Layout.rightMargin: Kirigami.Units.smallSpacing
                UqLabel {
                    id: nameLabel
                    objectName: "themeName"
                    text: chip.themeName
                    font.bold: chip.current
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                UqLabel {
                    objectName: "yoursTag"
                    visible: chip.isUser
                    text: "Yours"
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    opacity: 0.7
                }
                // Room for the menu button drawn over the tile's corner.
                Item {
                    visible: chip.isUser
                    implicitWidth: more.width - Kirigami.Units.smallSpacing
                }
            }
            UqLabel {
                objectName: "missingLine"
                visible: chip.theme.missing === true
                text: "Its files were removed"
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.negativeTextColor
                elide: Text.ElideRight
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.smallSpacing
            }
        }
        onClicked: {
            chip.chosen(chip.themeId);
            // The page's value is the truth: the tile snaps back if the switch is refused.
            checked = Qt.binding(() => chip.current);
        }
    }
    QQC2.ToolButton {
        id: more
        objectName: "themeMenuButton"
        visible: chip.isUser
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: Kirigami.Units.smallSpacing
        icon.name: "overflow-menu"
        display: QQC2.AbstractButton.IconOnly
        Accessible.name: "Options for theme " + chip.themeName
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.text: "Edit, export or delete"
        onClicked: menu.open()

        QQC2.Menu {
            id: menu
            y: more.height
            QQC2.MenuItem {
                objectName: "editItem"
                text: "Edit…"
                icon.name: "document-edit"
                onTriggered: chip.edit(chip.themeId)
            }
            QQC2.MenuItem {
                objectName: "exportItem"
                text: "Export…"
                icon.name: "document-export"
                onTriggered: chip.exportTheme(chip.themeId)
            }
            QQC2.MenuItem {
                objectName: "deleteItem"
                text: "Delete…"
                icon.name: "edit-delete"
                onTriggered: chip.remove(chip.themeId)
            }
        }
    }
}
