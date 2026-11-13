// Saved display arrangements: how the screens are set up when a particular set of screens is
// connected. A banner offers to remember the screens connected now ("Remember for these
// screens") or to update the arrangement that matches them ("Update “Desk”"). Below are the
// saved arrangements (Rename and Forget; Forget asks first) and the switch that applies one by
// itself when the screens change. Arrangement names are typed by people and screen names come
// from the screens, so they are shown through UqLabel (plain text), never as a stock control's
// `text:`; the dialog titles are fixed phrases.
//
// `arrangements` is `[{id, name, screens: [label], current}]`; `offer` is "remember", "update"
// or "" and `offerName` the name `update` would replace.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

ColumnLayout {
    id: root

    property var arrangements: []
    property string offer: ""
    property string offerName: ""
    property bool autoSwitch: true
    /// The last automatic switch was skipped for the arrangement that matches these screens.
    property bool failed: false
    /// Whether a request can be sent now (the page isn't busy).
    property bool idle: true
    property string highlight
    /// The arrangement being renamed or forgotten, until the dialog is answered.
    property string targetId: ""
    property string forgetId: ""
    property string forgetName: ""
    readonly property alias offerBanner: banner
    readonly property alias offerButton: offerBtn
    readonly property alias rememberButton: rememberBtn
    readonly property alias failedLabel: failedLine
    readonly property alias rows: list
    readonly property alias autoRow: autoSwitchRow
    readonly property alias nameField: nameBox
    readonly property alias forgetLabel: forgetWhat
    readonly property alias nameDialog: renameDialog
    readonly property alias forgetDialog: confirmForget
    readonly property bool naming: renameDialog.visible
    readonly property var current: arrangements.find(a => a.current === true) || null
    readonly property string offerText: offer === "update" ? "These screens are set up differently from the saved arrangement." : "You haven't saved how these screens are set up."
    readonly property string offerButtonText: offer === "update" ? "Update “" + offerName + "”" : "Remember for these screens"
    readonly property string failedText: current ? "“" + current.name + "” couldn't be used with these screens" : ""

    /// Save the screens connected now under `name`.
    signal remember(string name)
    signal rename(string id, string name)
    signal forget(string id)
    signal autoSwitchChosen(bool on)

    function startRemember() {
        targetId = "";
        nameBox.text = "";
        renameDialog.title = "Remember these screens";
        renameDialog.open();
    }
    function startRename(id, name) {
        targetId = id;
        nameBox.text = name;
        renameDialog.title = "Rename arrangement";
        renameDialog.open();
    }
    function saveName() {
        const text = nameBox.text.trim();
        if (!root.idle || text === "")
            return;
        if (root.targetId === "")
            root.remember(text);
        else
            root.rename(root.targetId, text);
        renameDialog.close();
    }
    function askForget(id, name) {
        forgetId = id;
        forgetName = name;
        confirmForget.open();
    }
    function confirmForgetNow() {
        const id = forgetId;
        forgetId = "";
        confirmForget.close();
        if (id !== "")
            root.forget(id);
    }
    function offerClicked() {
        if (root.offer === "update")
            root.remember(root.offerName);
        else
            root.startRemember();
    }

    spacing: Kirigami.Units.smallSpacing

    Rectangle {
        id: banner
        objectName: "offerBanner"
        visible: root.offer !== ""
        radius: Kirigami.Units.cornerRadius
        color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)
        implicitHeight: bannerRow.implicitHeight + Kirigami.Units.largeSpacing
        Layout.fillWidth: true

        RowLayout {
            id: bannerRow
            anchors.fill: parent
            anchors.margins: Kirigami.Units.smallSpacing
            spacing: Kirigami.Units.largeSpacing

            UqLabel {
                objectName: "offerText"
                text: root.offerText
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            ChoiceButton {
                id: offerBtn
                objectName: "offerButton"
                enabled: root.idle
                Accessible.name: root.offerButtonText
                label: root.offerButtonText
                onClicked: root.offerClicked()
                Keys.onReturnPressed: root.offerClicked()
                Keys.onEnterPressed: root.offerClicked()
            }
        }
    }

    SettingRow {
        settingId: "display-remember"
        iconName: "document-save"
        label: "Remember for these screens"
        highlight: root.highlight
        enabled: root.idle
        Layout.fillWidth: true

        QQC2.Button {
            id: rememberBtn
            objectName: "rememberButton"
            text: "Remember…"
            icon.name: "document-save"
            Accessible.name: "Remember these screens"
            onClicked: root.startRemember()
            Keys.onReturnPressed: root.startRemember()
            Keys.onEnterPressed: root.startRemember()
        }
    }

    UqLabel {
        objectName: "noArrangements"
        visible: root.arrangements.length === 0
        text: "No saved arrangements yet."
        opacity: 0.7
    }
    UqLabel {
        id: failedLine
        objectName: "failedLine"
        visible: root.failed && root.current !== null
        text: root.failedText
        wrapMode: Text.Wrap
        color: Kirigami.Theme.negativeTextColor
        Layout.fillWidth: true
    }
    Repeater {
        id: list
        model: root.arrangements
        RowLayout {
            id: row
            required property var modelData
            readonly property alias nameLabel: nameText
            readonly property alias forgetButton: forgetBtn
            readonly property alias renameButton: renameBtn
            objectName: "arrangement"
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing

            Kirigami.Icon {
                source: row.modelData.current === true ? "emblem-checked" : "preferences-desktop-display-randr"
                implicitWidth: Kirigami.Units.iconSizes.smallMedium
                implicitHeight: Kirigami.Units.iconSizes.smallMedium
                Accessible.ignored: true
            }
            ColumnLayout {
                spacing: 0
                Layout.fillWidth: true
                UqLabel {
                    id: nameText
                    objectName: "arrangementName"
                    text: String(row.modelData.name)
                    font.bold: row.modelData.current === true
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                UqLabel {
                    objectName: "arrangementScreens"
                    text: (row.modelData.current === true ? "In use now. " : "") + (row.modelData.screens || []).join(", ")
                    elide: Text.ElideRight
                    opacity: 0.7
                    Layout.fillWidth: true
                }
            }
            QQC2.Button {
                id: renameBtn
                objectName: "renameButton"
                text: "Rename"
                icon.name: "edit-rename"
                display: QQC2.AbstractButton.IconOnly
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: "Rename"
                enabled: root.idle
                Accessible.name: "Rename " + row.modelData.name
                onClicked: root.startRename(row.modelData.id, String(row.modelData.name))
            }
            QQC2.Button {
                id: forgetBtn
                objectName: "forgetButton"
                text: "Forget"
                icon.name: "edit-delete"
                display: QQC2.AbstractButton.IconOnly
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: "Forget"
                enabled: root.idle
                Accessible.name: "Forget " + row.modelData.name
                onClicked: root.askForget(row.modelData.id, String(row.modelData.name))
            }
        }
    }

    SwitchRow {
        id: autoSwitchRow
        settingId: "display-arrangements-auto"
        iconName: "media-playlist-shuffle"
        label: "Switch arrangements when screens change"
        highlight: root.highlight
        enabled: root.idle
        value: root.autoSwitch
        onChosen: v => root.autoSwitchChosen(v)
        Layout.fillWidth: true
    }

    // Remember and Rename share one dialog; its title is one of two fixed phrases.
    Kirigami.Dialog {
        id: renameDialog
        title: "Remember these screens"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 22
        onOpened: nameBox.forceActiveFocus()
        customFooterActions: [
            Kirigami.Action {
                text: "Save"
                icon.name: "document-save"
                enabled: root.idle && nameBox.text.trim() !== ""
                onTriggered: root.saveName()
            }
        ]

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing
            UqLabel {
                text: "Name"
            }
            QQC2.TextField {
                id: nameBox
                objectName: "nameField"
                maximumLength: 60
                placeholderText: "For example: Desk"
                Accessible.name: "Name"
                Layout.fillWidth: true
                onAccepted: root.saveName()
            }
        }
    }

    Kirigami.PromptDialog {
        id: confirmForget
        title: "Forget this arrangement"
        standardButtons: Kirigami.Dialog.Cancel
        onClosed: root.forgetId = ""
        customFooterActions: [
            Kirigami.Action {
                text: "Forget"
                icon.name: "edit-delete"
                enabled: root.idle
                onTriggered: root.confirmForgetNow()
            }
        ]

        ColumnLayout {
            UqLabel {
                id: forgetWhat
                objectName: "forgetWhat"
                text: "Forget “" + root.forgetName + "”? Your screens stay as they are."
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
        }
    }
}
