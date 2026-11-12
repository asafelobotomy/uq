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

QQC2.ScrollView {
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

    Accessible.role: Accessible.Pane
    Accessible.name: "Users"
    contentWidth: availableWidth

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

    ColumnLayout {
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 44)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Users"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        ErrorCard {
            objectName: "notAvailable"
            visible: page.service !== null
            title: "Not available"
            detail: page.service ? String(page.service.text || "") : ""
            Layout.fillWidth: true
        }
        RuleMessage {
            objectName: "usersError"
            negative: true
            closable: false
            lines: um.error !== "" ? ["That wasn't done: " + um.error] : []
            Layout.fillWidth: true
        }
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }

        SettingRow {
            settingId: "user-accounts"
            label: "People who can log in"
            highlight: App.highlight
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            QQC2.Button {
                objectName: "addUser"
                text: "Add a user…"
                icon.name: "list-add-user"
                enabled: page.idle
                onClicked: addDialog.open()
            }
        }

        Repeater {
            model: page.users
            delegate: Kirigami.AbstractCard {
                id: card
                required property var modelData
                readonly property var u: modelData
                Layout.fillWidth: true

                contentItem: ColumnLayout {
                    spacing: Kirigami.Units.smallSpacing

                    RowLayout {
                        spacing: Kirigami.Units.largeSpacing
                        Layout.fillWidth: true

                        UqAvatar {
                            path: card.u.icon
                            name: page.display(card.u)
                            Accessible.name: "Picture of " + page.display(card.u)
                        }
                        ColumnLayout {
                            spacing: 0
                            Layout.fillWidth: true
                            UqLabel {
                                text: page.display(card.u) + (card.u.me ? " (you)" : "")
                                font.bold: true
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                            UqLabel {
                                text: card.u.name + " · " + (card.u.admin ? "Administrator" : "Standard user") + (card.u.locked ? " · locked" : "")
                                opacity: 0.7
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                        }
                    }
                    Flow {
                        spacing: Kirigami.Units.smallSpacing
                        Layout.fillWidth: true

                        QQC2.Button {
                            text: "Change name…"
                            icon.name: "edit-rename"
                            enabled: page.idle
                            onClicked: {
                                nameDialog.uid = card.u.uid;
                                nameDialog.current = String(card.u.real_name || "");
                                nameDialog.open();
                            }
                        }
                        QQC2.Button {
                            text: "Change password…"
                            icon.name: "dialog-password"
                            enabled: page.idle
                            onClicked: {
                                passwordDialog.uid = card.u.uid;
                                passwordDialog.who = page.display(card.u);
                                passwordDialog.mine = !!card.u.me;
                                passwordDialog.open();
                            }
                        }
                        QQC2.Button {
                            text: "Change picture…"
                            icon.name: "insert-image"
                            enabled: page.idle
                            onClicked: {
                                avatarDialog.uid = card.u.uid;
                                avatarDialog.open();
                            }
                        }
                        QQC2.Switch {
                            id: adminSwitch
                            text: "Administrator"
                            checked: !!card.u.admin
                            enabled: page.idle && (card.u.admin ? card.u.demote_problem === "" : true)
                            onToggled: {
                                um.setAdmin(card.u.uid, adminSwitch.checked);
                                adminSwitch.checked = Qt.binding(() => !!card.u.admin);
                            }
                        }
                        QQC2.Button {
                            text: "Remove…"
                            icon.name: "list-remove-user"
                            enabled: page.idle && card.u.remove_problem === ""
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
                    }
                }
            }
        }

        SettingRow {
            settingId: "users-auto-login"
            label: page.autoLogin.user ? "Auto-login: on for " + page.autoLogin.user : "Auto-login: off"
            highlight: App.highlight
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            QQC2.Button {
                objectName: "openLoginScreen"
                text: "Login screen settings…"
                icon.name: "go-next"
                visible: !!page.autoLogin.greeter
                onClicked: App.showPage("login-screen", "auto-login")
            }
        }
        UqLabel {
            visible: page.loaded && !page.autoLogin.greeter
            text: "Auto-login is set on the Login screen page, which appears when uQ is your login screen."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
