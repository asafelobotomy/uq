// "Add an app" or "Add a command" for the Startup page: an installed app chosen from a list, or
// a command typed with a name. App names come from other packages' files, so the list shows
// them through UqLabel only. The title is one of two fixed phrases.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

Kirigami.Dialog {
    id: dialog

    /// The installed apps `[{id, name, icon}]`.
    property var apps: []
    /// "app" or "command".
    property string mode: "app"
    /// Whether a request can be sent now (the page isn't busy).
    property bool idle: true
    property string chosenId
    readonly property var shown: {
        const q = filterBox.text.trim().toLowerCase();
        return q === "" ? dialog.apps : dialog.apps.filter(a => String(a.name).toLowerCase().indexOf(q) >= 0);
    }
    readonly property bool complete: mode === "app" ? chosenId !== "" : commandBox.text.trim() !== ""
    readonly property alias commandField: commandBox
    readonly property alias nameField: nameBox

    /// An installed app was chosen, by desktop id.
    signal appChosen(string id)
    /// A command was typed; `name` is "" for "name it after its program".
    signal commandChosen(string name, string command)

    function start(mode) {
        dialog.mode = mode;
        dialog.chosenId = "";
        filterBox.clear();
        nameBox.clear();
        commandBox.clear();
        dialog.open();
    }
    function add() {
        if (!dialog.idle || !dialog.complete)
            return;
        if (dialog.mode === "app")
            dialog.appChosen(dialog.chosenId);
        else
            dialog.commandChosen(nameBox.text.trim(), commandBox.text.trim());
        dialog.close();
    }

    title: mode === "app" ? "Add an app" : "Add a command"
    standardButtons: Kirigami.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 26
    onOpened: (dialog.mode === "app" ? filterBox : commandBox).forceActiveFocus()
    customFooterActions: [
        Kirigami.Action {
            text: "Add"
            icon.name: "list-add"
            enabled: dialog.idle && dialog.complete
            onTriggered: dialog.add()
        }
    ]

    ColumnLayout {
        spacing: Kirigami.Units.smallSpacing

        QQC2.TextField {
            id: filterBox
            visible: dialog.mode === "app"
            placeholderText: "Search apps"
            Accessible.name: "Search apps"
            Layout.fillWidth: true
        }
        ListView {
            id: list
            visible: dialog.mode === "app"
            clip: true
            model: dialog.shown
            Layout.fillWidth: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 14
            QQC2.ScrollBar.vertical: QQC2.ScrollBar {}
            delegate: QQC2.ItemDelegate {
                id: item
                required property var modelData
                width: ListView.view.width
                highlighted: dialog.chosenId === String(modelData.id)
                Accessible.name: String(modelData.name)
                contentItem: RowLayout {
                    spacing: Kirigami.Units.smallSpacing
                    Kirigami.Icon {
                        source: String(item.modelData.icon || "") !== "" ? String(item.modelData.icon) : "application-x-executable"
                        implicitWidth: Kirigami.Units.iconSizes.small
                        implicitHeight: Kirigami.Units.iconSizes.small
                        Accessible.ignored: true
                    }
                    UqLabel {
                        text: String(item.modelData.name)
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }
                onClicked: dialog.chosenId = String(modelData.id)
                onDoubleClicked: {
                    dialog.chosenId = String(modelData.id);
                    dialog.add();
                }
            }
        }
        UqLabel {
            visible: dialog.mode === "app" && dialog.shown.length === 0
            text: "No installed app has that name."
            opacity: 0.7
        }

        UqLabel {
            visible: dialog.mode === "command"
            text: "Name"
        }
        QQC2.TextField {
            id: nameBox
            visible: dialog.mode === "command"
            maximumLength: 100
            placeholderText: "Such as Backup check"
            Accessible.name: "Name"
            Layout.fillWidth: true
        }
        UqLabel {
            visible: dialog.mode === "command"
            text: "Command"
        }
        QQC2.TextField {
            id: commandBox
            visible: dialog.mode === "command"
            placeholderText: "Such as backup-check --quiet"
            Accessible.name: "Command"
            Layout.fillWidth: true
            onAccepted: dialog.add()
        }
        UqLabel {
            visible: dialog.mode === "command"
            text: "It runs directly, not in a terminal. Put double quotes around a part with spaces."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }
    }
}
