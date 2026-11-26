// Users' main tab: each person, with their name, picture, password and auto-login.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    // Passed in from UsersPage.qml.
    property var addDialog
    property var avatarDialog
    property var nameDialog
    property var page
    property var passwordDialog
    property var removeDialog
    property var um

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
