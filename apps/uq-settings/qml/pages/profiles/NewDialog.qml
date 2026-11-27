// Profiles' dialog for a new profile: from this page's settings or from recent changes.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Kirigami.Dialog {
    id: newDialog
    // Passed in from ProfilesPage.qml.
    property var page
    property var pm

    /// The ids of the pages ticked under "The current settings of these pages".
    property var pickedPages: []
    readonly property bool fromPages: fromPagesButton.checked
    readonly property bool canCreate: page.idle && newName.text.trim() !== "" && (!fromPages || pickedPages.length > 0)

    function start() {
        newName.clear();
        newIcon.chosen = page.icons.length > 0 ? page.icons[0] : "";
        fromHistoryButton.checked = true;
        period.currentIndex = 1;
        newDialog.pickedPages = [];
        newDialog.open();
    }
    function create() {
        if (!newDialog.canCreate)
            return;
        if (newDialog.fromPages)
            pm.createFromPages(newName.text, newIcon.chosen, JSON.stringify(newDialog.pickedPages));
        else
            pm.createFromHistory(newName.text, newIcon.chosen, page.sinceOf(period.currentIndex));
        newDialog.close();
    }

    title: "New profile"
    onOpened: newName.forceActiveFocus()
    standardButtons: Kirigami.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 26
    customFooterActions: [
        Kirigami.Action {
            text: "Create"
            icon.name: "list-add"
            enabled: newDialog.canCreate
            onTriggered: newDialog.create()
        }
    ]

    ColumnLayout {
        spacing: Kirigami.Units.smallSpacing

        UqLabel {
            text: "Name"
        }
        QQC2.TextField {
            id: newName
            objectName: "newProfileName"
            maximumLength: 100
            placeholderText: "Such as Gaming or Presentation"
            Accessible.name: "Name of the new profile"
            Layout.fillWidth: true
            onAccepted: newDialog.create()
        }
        UqLabel {
            text: "Icon"
            Layout.topMargin: Kirigami.Units.smallSpacing
        }
        IconChoice {
            id: newIcon
            icons: page.icons
            names: page.iconNames
            Layout.fillWidth: true
        }
        UqLabel {
            text: "Start with"
            Layout.topMargin: Kirigami.Units.smallSpacing
        }
        QQC2.ButtonGroup {
            id: sourceGroup
        }
        RowLayout {
            QQC2.RadioButton {
                id: fromHistoryButton
                text: "Settings I changed"
                checked: true
                QQC2.ButtonGroup.group: sourceGroup
            }
            UqComboBox {
                id: period
                model: page.periods
                currentIndex: 1
                enabled: fromHistoryButton.checked
                Accessible.name: "Changed in"
            }
        }
        QQC2.RadioButton {
            id: fromPagesButton
            text: "The current settings of these pages"
            QQC2.ButtonGroup.group: sourceGroup
        }
        Repeater {
            model: page.pages
            delegate: RowLayout {
                id: pageRow

                required property var modelData
                readonly property bool picked: newDialog.pickedPages.indexOf(modelData.id) >= 0

                enabled: newDialog.fromPages
                Layout.leftMargin: Kirigami.Units.largeSpacing * 2

                QQC2.CheckBox {
                    id: pageBox
                    checked: pageRow.picked
                    Accessible.name: pageRow.modelData.name
                    onToggled: {
                        const others = newDialog.pickedPages.filter(id => id !== pageRow.modelData.id);
                        newDialog.pickedPages = pageBox.checked ? others.concat([pageRow.modelData.id]) : others;
                        pageBox.checked = Qt.binding(() => pageRow.picked);
                    }
                }
                UqLabel {
                    text: pageRow.modelData.name
                    MouseArea {
                        anchors.fill: parent
                        enabled: pageBox.enabled
                        onClicked: {
                            pageBox.toggle();
                            pageBox.toggled();
                        }
                    }
                }
            }
        }
    }
}
