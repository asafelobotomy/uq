// One theme on the Appearance page: a button that switches to it, with its name, a "Yours" tag for
// the user's own themes and a line for one whose files were removed. The user's themes also have a
// menu: Edit, Export and Delete. Names come from theme files anyone can write, so they are shown
// through UqLabel (plain text), never as a stock control's `text:`.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

RowLayout {
    id: chip

    /// `{id, name, user, active, missing}`.
    property var theme: ({})
    readonly property string themeId: String(theme.id ?? "")
    readonly property string themeName: String(theme.name ?? "")
    readonly property bool current: theme.active === true
    readonly property bool isUser: theme.user === true

    /// The theme was chosen: switch to it.
    signal chosen(string id)
    /// **Edit…**, **Export…** and **Delete…** (the user's themes only).
    signal edit(string id)
    signal exportTheme(string id)
    signal remove(string id)

    spacing: 0
    Accessible.role: Accessible.Grouping
    Accessible.name: "Theme " + chip.themeName

    QQC2.Button {
        id: pick
        objectName: "themeButton"
        checkable: true
        checked: chip.current
        highlighted: chip.current
        Accessible.name: "Theme " + chip.themeName
        contentItem: ColumnLayout {
            spacing: 0
            UqLabel {
                objectName: "themeName"
                text: chip.themeName
                font.bold: chip.current
                // No accent colour of its own: the current theme's button is already checked and
                // highlighted, and accent text on an accent fill would vanish. As LookStep.
                elide: Text.ElideRight
                horizontalAlignment: Text.AlignHCenter
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 12
            }
            UqLabel {
                objectName: "yoursTag"
                visible: chip.isUser
                text: "Yours"
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                opacity: 0.7
                horizontalAlignment: Text.AlignHCenter
                Layout.fillWidth: true
            }
            UqLabel {
                objectName: "missingLine"
                visible: chip.theme.missing === true
                text: "Its files were removed"
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.negativeTextColor
                horizontalAlignment: Text.AlignHCenter
                Layout.fillWidth: true
            }
        }
        onClicked: {
            chip.chosen(chip.themeId);
            // The page's value is the truth: the button snaps back if the switch is refused.
            checked = Qt.binding(() => chip.current);
        }
    }
    QQC2.ToolButton {
        id: more
        objectName: "themeMenuButton"
        visible: chip.isUser
        icon.name: "overflow-menu"
        display: QQC2.AbstractButton.IconOnly
        Accessible.name: "Options for theme " + chip.themeName
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
