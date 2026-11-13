// Users (spec §2.6, §3): the people who can log in to this computer. Changes go through the
// accounts service, which asks for an administrator password itself; a cancelled prompt leaves
// everything as it was. Your own password is changed with your current one (through passwd),
// without an administrator. Removing an account needs its user name typed; you can't remove
// yourself or the last administrator, or make the last administrator a standard user. Password
// boxes are SecretFields, emptied after every attempt. Names come from the system and are shown
// through UqLabel (plain text) only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"

SettingsPage {
    id: page

    readonly property var snap: {
        try {
            return JSON.parse(um.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property bool loaded: snap.users !== undefined
    readonly property var service: snap.service || null
    readonly property bool ready: page.loaded && page.service === null
    readonly property bool idle: page.ready && !um.busy
    readonly property var users: snap.users || []
    readonly property var autoLogin: snap.auto_login || ({})
    property int noticeSeen: 0

    function refresh() {
        um.refresh();
    }
    function showNews() {
        const notice = page.snap.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.show(notice.text !== "" ? [notice.text] : []);
        }
    }
    function display(u) {
        return u.real_name && u.real_name !== "" ? String(u.real_name) : String(u.name);
    }

    UsersModel {
        id: um
        onStateJsonChanged: page.showNews()
    }

    FileDialog {
        id: avatarDialog
        property int uid: -1
        title: "Choose a picture for the account"
        nameFilters: ["Pictures (*.png *.jpg *.jpeg)"]
        onAccepted: um.setAvatar(avatarDialog.uid, String(selectedFile))
    }

    Kirigami.Dialog {
        id: nameDialog
        property int uid: -1
        property string current
        title: "Change the name"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 24
        onOpened: {
            nameField.text = nameDialog.current;
            nameField.forceActiveFocus();
        }
        customFooterActions: [
            Kirigami.Action {
                text: "Save"
                icon.name: "document-save"
                enabled: page.idle
                onTriggered: {
                    um.setRealName(nameDialog.uid, nameField.text);
                    nameDialog.close();
                }
            }
        ]
        QQC2.TextField {
            id: nameField
            maximumLength: page.snap.limits ? page.snap.limits.real_name : 255
            Accessible.name: "Full name"
        }
    }

    Kirigami.Dialog {
        id: passwordDialog
        property int uid: -1
        property string who
        // Your own account: passwd checks your current password, and no administrator is asked.
        property bool mine: false
        title: "Change the password"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 24
        onOpened: (mine ? currentPassword : newPassword).forceActiveFocus()
        onClosed: {
            currentPassword.wipe();
            newPassword.wipe();
            confirmPassword.wipe();
        }
        customFooterActions: [
            Kirigami.Action {
                text: "Change"
                icon.name: "dialog-password"
                enabled: page.idle && newPassword.text !== "" && newPassword.text === confirmPassword.text
                         && (!passwordDialog.mine || currentPassword.text !== "")
                onTriggered: {
                    um.setPassword(passwordDialog.uid, passwordDialog.mine ? currentPassword.text : "",
                                   newPassword.text, confirmPassword.text);
                    passwordDialog.close();
                }
            }
        ]
        ColumnLayout {
            UqLabel {
                visible: passwordDialog.mine
                text: "Your current password"
            }
            SecretField {
                id: currentPassword
                objectName: "currentPassword"
                visible: passwordDialog.mine
                Accessible.name: "Current password"
                Layout.fillWidth: true
            }
            UqLabel {
                text: passwordDialog.mine ? "Your new password" : "New password for " + passwordDialog.who
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            SecretField {
                id: newPassword
                objectName: "newPassword"
                Accessible.name: "New password"
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Type it again"
            }
            SecretField {
                id: confirmPassword
                objectName: "confirmPassword"
                Accessible.name: "Confirm the new password"
                Layout.fillWidth: true
            }
            UqLabel {
                visible: confirmPassword.text !== "" && newPassword.text !== confirmPassword.text
                text: "The two passwords don't match."
                color: Kirigami.Theme.negativeTextColor
            }
        }
    }

    Kirigami.Dialog {
        id: addDialog
        title: "Add a user"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 26
        onOpened: addRealName.forceActiveFocus()
        onClosed: {
            addPassword.wipe();
            addConfirm.wipe();
        }
        customFooterActions: [
            Kirigami.Action {
                text: "Add"
                icon.name: "list-add-user"
                enabled: page.idle && addName.text.trim() !== "" && addPassword.text !== "" && addPassword.text === addConfirm.text
                onTriggered: {
                    um.addUser(addName.text.trim(), addRealName.text, addAdmin.checked, addPassword.text, addConfirm.text);
                    addRealName.clear();
                    addName.clear();
                    addAdmin.checked = false;
                    addDialog.close();
                }
            }
        ]
        GridLayout {
            columns: 2
            UqLabel {
                text: "Full name"
            }
            QQC2.TextField {
                id: addRealName
                Accessible.name: "Full name"
                Layout.fillWidth: true
                onTextEdited: {
                    if (!addName.edited)
                        addName.text = addRealName.text.toLowerCase().replace(/[^a-z0-9_-]/g, "").replace(/^[^a-z_]+/, "").slice(0, 32);
                }
            }
            UqLabel {
                text: "User name"
            }
            QQC2.TextField {
                id: addName
                property bool edited: false
                maximumLength: 32
                Accessible.name: "User name"
                Layout.fillWidth: true
                onTextEdited: addName.edited = true
            }
            UqLabel {
                text: "Password"
            }
            SecretField {
                id: addPassword
                Accessible.name: "Password"
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Type it again"
            }
            SecretField {
                id: addConfirm
                Accessible.name: "Confirm the password"
                Layout.fillWidth: true
            }
            Item {
                implicitWidth: 1
            }
            QQC2.CheckBox {
                id: addAdmin
                text: "Administrator: can change system settings and other accounts"
            }
        }
    }

    Kirigami.Dialog {
        id: removeDialog
        property int uid: -1
        property string userName
        title: "Remove this account?"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 26
        onOpened: {
            typedName.clear();
            removeFiles.checked = false;
            typedName.forceActiveFocus();
        }
        customFooterActions: [
            Kirigami.Action {
                text: "Remove"
                icon.name: "list-remove-user"
                enabled: page.idle && typedName.text === removeDialog.userName
                onTriggered: {
                    um.removeUser(removeDialog.uid, typedName.text, removeFiles.checked);
                    removeDialog.close();
                }
            }
        ]
        ColumnLayout {
            UqLabel {
                text: "To remove " + removeDialog.userName + ", type the user name below. This can't be undone."
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            QQC2.TextField {
                id: typedName
                objectName: "typedName"
                Accessible.name: "Type the user name to confirm"
                Layout.fillWidth: true
            }
            QQC2.CheckBox {
                id: removeFiles
                text: "Also delete their home folder and files"
            }
        }
    }

    banners: [
        ErrorCard {
            objectName: "notAvailable"
            visible: page.service !== null
            title: "Not available"
            detail: page.service ? String(page.service.text || "") : ""
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "usersError"
            negative: true
            closable: false
            lines: um.error !== "" ? ["That wasn't done: " + um.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "Users"
        iconName: "system-users"

        SettingsCard {
            title: "People who can log in"
            iconName: "system-users"

            SettingRow {
                settingId: "user-accounts"
                iconName: "user-identity"
                label: page.users.length === 1 ? "1 account" : page.users.length + " accounts"
                highlight: App.highlight
                Layout.fillWidth: true

                QQC2.Button {
                    objectName: "addUser"
                    text: "Add a user…"
                    icon.name: "list-add-user"
                    enabled: page.idle
                    onClicked: addDialog.open()
                }
            }
            SettingRow {
                settingId: "users-auto-login"
                iconName: "system-switch-user"
                label: page.autoLogin.user ? "Logs in automatically as " + page.autoLogin.user : "No automatic login"
                help: page.loaded && !page.autoLogin.greeter ? "Automatic login is set on the Login screen page, which appears when uQ is your login screen." : ""
                highlight: App.highlight
                Layout.fillWidth: true

                QQC2.Button {
                    objectName: "openLoginScreen"
                    text: "Login screen"
                    icon.name: "go-next"
                    visible: !!page.autoLogin.greeter
                    Accessible.name: "Open the Login screen settings"
                    onClicked: App.showPage("login-screen", "auto-login")
                }
            }
        }

        // One card per account: the picture (click to change it), the name, and what it can do.
        Repeater {
            model: page.users
            delegate: SettingsCard {
                id: card
                required property var modelData
                readonly property var u: modelData
                readonly property string who: page.display(card.u)
                objectName: "user-" + card.u.name
                Accessible.name: card.who

                RowLayout {
                    spacing: Kirigami.Units.largeSpacing
                    Layout.fillWidth: true

                    QQC2.AbstractButton {
                        id: pictureButton
                        objectName: "changePicture"
                        enabled: page.idle
                        hoverEnabled: true
                        implicitWidth: Kirigami.Units.iconSizes.huge
                        implicitHeight: Kirigami.Units.iconSizes.huge
                        Accessible.role: Accessible.Button
                        Accessible.name: "Change the picture of " + card.who
                        QQC2.ToolTip.visible: hovered || activeFocus
                        QQC2.ToolTip.text: "Change the picture"
                        onClicked: {
                            avatarDialog.uid = card.u.uid;
                            avatarDialog.open();
                        }
                        contentItem: Item {
                            UqAvatar {
                                anchors.fill: parent
                                path: card.u.icon
                                name: card.who
                                Accessible.name: "Picture of " + card.who
                            }
                            // A camera badge on hover or focus: the picture can be changed.
                            Rectangle {
                                visible: pictureButton.enabled && (pictureButton.hovered || pictureButton.activeFocus)
                                anchors.right: parent.right
                                anchors.bottom: parent.bottom
                                width: Kirigami.Units.iconSizes.smallMedium + Kirigami.Units.smallSpacing * 2
                                height: width
                                radius: width / 2
                                color: Kirigami.Theme.backgroundColor
                                border.width: 1
                                border.color: Kirigami.Theme.highlightColor
                                Kirigami.Icon {
                                    anchors.centerIn: parent
                                    source: "insert-image"
                                    implicitWidth: Kirigami.Units.iconSizes.small
                                    implicitHeight: Kirigami.Units.iconSizes.small
                                    Accessible.ignored: true
                                }
                            }
                        }
                    }
                    ColumnLayout {
                        spacing: Kirigami.Units.smallSpacing
                        Layout.fillWidth: true

                        RowLayout {
                            spacing: Kirigami.Units.smallSpacing
                            Layout.fillWidth: true
                            UqLabel {
                                objectName: "userName"
                                text: card.who
                                font.bold: true
                                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.3
                                elide: Text.ElideRight
                                Layout.maximumWidth: Kirigami.Units.gridUnit * 20
                            }
                            // "You", as a small tag.
                            Rectangle {
                                visible: !!card.u.me
                                implicitWidth: youLabel.implicitWidth + Kirigami.Units.largeSpacing
                                implicitHeight: youLabel.implicitHeight + Kirigami.Units.smallSpacing
                                radius: height / 2
                                color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.2)
                                UqLabel {
                                    id: youLabel
                                    anchors.centerIn: parent
                                    text: "You"
                                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                                }
                            }
                            Item {
                                Layout.fillWidth: true
                            }
                        }
                        RowLayout {
                            spacing: Kirigami.Units.smallSpacing
                            Kirigami.Icon {
                                source: "im-user"
                                implicitWidth: Kirigami.Units.iconSizes.small
                                implicitHeight: Kirigami.Units.iconSizes.small
                                opacity: 0.7
                                Accessible.ignored: true
                            }
                            UqLabel {
                                text: String(card.u.name)
                                opacity: 0.7
                            }
                            Kirigami.Icon {
                                visible: !!card.u.locked
                                source: "emblem-locked"
                                implicitWidth: Kirigami.Units.iconSizes.small
                                implicitHeight: Kirigami.Units.iconSizes.small
                                Layout.leftMargin: Kirigami.Units.largeSpacing
                                Accessible.ignored: true
                            }
                            UqLabel {
                                visible: !!card.u.locked
                                text: "Locked"
                                opacity: 0.7
                            }
                        }
                        RowLayout {
                            spacing: Kirigami.Units.smallSpacing
                            Kirigami.Icon {
                                source: card.u.admin ? "security-high" : "user-identity"
                                implicitWidth: Kirigami.Units.iconSizes.small
                                implicitHeight: Kirigami.Units.iconSizes.small
                                Accessible.ignored: true
                            }
                            QQC2.Switch {
                                id: adminSwitch
                                objectName: "adminSwitch"
                                text: "Administrator"
                                checked: !!card.u.admin
                                enabled: page.idle && (card.u.admin ? card.u.demote_problem === "" : true)
                                Accessible.description: card.who
                                QQC2.ToolTip.visible: hovered
                                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                                QQC2.ToolTip.text: "Can change system settings and other accounts"
                                onToggled: {
                                    um.setAdmin(card.u.uid, adminSwitch.checked);
                                    adminSwitch.checked = Qt.binding(() => !!card.u.admin);
                                }
                            }
                        }
                    }
                }
                Kirigami.Separator {
                    Layout.fillWidth: true
                }
                RowLayout {
                    spacing: Kirigami.Units.smallSpacing
                    Layout.fillWidth: true

                    QQC2.ToolButton {
                        text: "Name…"
                        icon.name: "edit-rename"
                        enabled: page.idle
                        Accessible.name: "Change the name"
                        Accessible.description: card.who
                        onClicked: {
                            nameDialog.uid = card.u.uid;
                            nameDialog.current = String(card.u.real_name || "");
                            nameDialog.open();
                        }
                    }
                    QQC2.ToolButton {
                        text: "Password…"
                        icon.name: "dialog-password"
                        enabled: page.idle
                        Accessible.name: "Change the password"
                        Accessible.description: card.who
                        onClicked: {
                            passwordDialog.uid = card.u.uid;
                            passwordDialog.who = card.who;
                            passwordDialog.mine = !!card.u.me;
                            passwordDialog.open();
                        }
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                    QQC2.ToolButton {
                        text: "Remove…"
                        icon.name: "list-remove-user"
                        enabled: page.idle && card.u.remove_problem === ""
                        Accessible.name: "Remove this account"
                        Accessible.description: card.u.remove_problem !== "" ? String(card.u.remove_problem) : card.who
                        QQC2.ToolTip.visible: hovered && card.u.remove_problem !== ""
                        QQC2.ToolTip.text: String(card.u.remove_problem)
                        onClicked: {
                            removeDialog.uid = card.u.uid;
                            removeDialog.userName = String(card.u.name);
                            removeDialog.open();
                        }
                    }
                }
                UqLabel {
                    visible: card.u.remove_problem !== "" && !card.u.me
                    text: String(card.u.remove_problem)
                    wrapMode: Text.Wrap
                    opacity: 0.7
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.smallSpacing
                }
            }
        }
    }
}
