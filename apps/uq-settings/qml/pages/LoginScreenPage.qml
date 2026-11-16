// Login screen (Phase 5 §4.1): how uQ's login screen looks, and auto-login. Shown only when uQ is
// the login screen. Every change goes through LoginScreenModel to uQ's system settings helper,
// which asks for an administrator password; a cancelled prompt leaves the control as it was.
// "Same as mine" choices are copied when saved. Theme names, user names and file paths are shown
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
            return JSON.parse(lm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property bool loaded: snap.greeter !== undefined
    readonly property bool canSave: page.loaded && snap.helper === true
    readonly property bool idle: page.canSave && !lm.busy
    readonly property var greeter: snap.greeter || ({})
    readonly property var variants: snap.variants || []
    readonly property var mine: snap.mine || null
    readonly property var autoLogin: snap.auto_login || ({})
    readonly property string me: String(autoLogin.me || "")
    readonly property string autoUser: String(autoLogin.user || "")
    property int noticeSeen: 0

    function variantName(v) {
        const t = String(v.theme || "");
        const name = t.length > 0 ? t.charAt(0).toUpperCase() + t.slice(1) : "";
        return name + ", " + (v.variant === "light" ? "light" : "dark");
    }
    readonly property var themeLabels: {
        const first = page.mine ? "Same as mine (" + page.variantName(page.mine) + ")" : "Same as mine";
        return [first].concat(page.variants.map(v => page.variantName(v)));
    }
    /// The greeter's theme in the list; "Same as mine" (0) when it is none of the listed ones,
    /// as when it was never set.
    function themeIndex() {
        const i = page.variants.findIndex(v => v.theme === page.greeter.theme && v.variant === page.greeter.variant);
        return i < 0 ? 0 : i + 1;
    }
    /// The theme and variant the login screen shows, for the preview.
    readonly property var shownLook: {
        const i = page.variants.findIndex(v => v.theme === page.greeter.theme && v.variant === page.greeter.variant);
        if (i >= 0)
            return page.variants[i];
        return page.mine || page.greeter;
    }
    readonly property bool previewDark: page.shownLook.variant !== "light"
    // The preview's colours, from this app's own palette: its darker and lighter ends.
    readonly property bool appDark: Kirigami.ColorUtils.brightnessForColor(Kirigami.Theme.backgroundColor) === Kirigami.ColorUtils.Dark
    readonly property color darkEnd: page.appDark ? Kirigami.Theme.backgroundColor : Kirigami.Theme.textColor
    readonly property color lightEnd: page.appDark ? Kirigami.Theme.textColor : Kirigami.Theme.backgroundColor
    readonly property bool wallpaperSet: !!(page.snap.wallpaper && page.snap.wallpaper.set)
    /// Bumped on every new state, so the preview reads the picture again.
    property int pictureSeq: 0
    /// Save the look with one thing changed.
    function saveLook(theme, variant, hideUsers, reduceMotion) {
        lm.setAppearance(theme, variant, hideUsers, reduceMotion);
    }

    function refresh() {
        lm.refresh();
    }
    function showNews() {
        const notice = page.snap.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.show(notice.text !== "" ? [notice.text] : []);
        }
    }

    LoginScreenModel {
        id: lm
        onStateJsonChanged: {
            page.showNews();
            page.pictureSeq++;
        }
    }

    FileDialog {
        id: pictureDialog
        title: "Choose a picture for the login screen"
        nameFilters: ["Pictures (*.png *.jpg *.jpeg *.webp)"]
        onAccepted: lm.wallpaperFile(String(selectedFile))
    }

    headerItems: [
        QQC2.BusyIndicator {
            visible: lm.busy
            running: lm.busy
            implicitWidth: Kirigami.Units.iconSizes.medium
            implicitHeight: Kirigami.Units.iconSizes.medium
            Accessible.name: "Waiting for the password prompt"
        }
    ]
    banners: [
        ErrorCard {
            objectName: "helperMissing"
            visible: page.loaded && !page.canSave
            title: "These settings can only be viewed here"
            detail: String(page.snap.helper_note || "")
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "loginError"
            negative: true
            closable: false
            lines: lm.error !== "" ? ["That wasn't saved: " + lm.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            objectName: "loginNotice"
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "Login screen"
        iconName: "preferences-system-login"

        // What the login screen will look like beside what sets it: its picture or theme
        // colour, the clock and the sign-in box. A sketch in this app's colours, light or dark as
        // the chosen theme.
        SettingsCard {
            title: "Appearance"
            iconName: "preferences-desktop-theme"
            help: "Changes here ask for an administrator password."
            wide: true

            // The preview beside its controls, or above them in a narrow window.
            GridLayout {
                id: appearanceRow
                columns: appearanceRow.width >= Kirigami.Units.gridUnit * 44 ? 2 : 1
                columnSpacing: Kirigami.Units.largeSpacing * 2
                rowSpacing: Kirigami.Units.largeSpacing
                Layout.fillWidth: true

                Rectangle {
                    id: preview
                    objectName: "greeterPreview"
                    readonly property color bg: page.previewDark ? page.darkEnd : page.lightEnd
                    readonly property color fg: page.previewDark ? page.lightEnd : page.darkEnd
                    Layout.preferredWidth: Math.min(Kirigami.Units.gridUnit * 24, appearanceRow.width)
                    Layout.preferredHeight: Layout.preferredWidth * 9 / 16
                    Layout.alignment: Qt.AlignTop
                    radius: Kirigami.Units.cornerRadius * 2
                    color: preview.bg
                    clip: true
                    Accessible.role: Accessible.Graphic
                    Accessible.name: "Preview of the login screen: " + (page.shownLook.theme ? page.variantName(page.shownLook) : "") + (page.wallpaperSet ? ", with a picture" : "")

                    Image {
                        id: picture
                        anchors.fill: parent
                        visible: page.wallpaperSet && status === Image.Ready
                        source: page.wallpaperSet ? "file:///var/lib/uq-greeter/wallpaper?" + page.pictureSeq : ""
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                        cache: false
                        sourceSize.width: Math.round(preview.width * Screen.devicePixelRatio)
                    }
                    ColumnLayout {
                        anchors.centerIn: parent
                        spacing: Kirigami.Units.largeSpacing

                        UqLabel {
                            text: Qt.formatTime(new Date(), "HH:mm")
                            color: picture.visible ? Kirigami.Theme.highlightedTextColor : preview.fg
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 3
                            font.weight: Font.Light
                            Layout.alignment: Qt.AlignHCenter
                            Accessible.ignored: true
                        }
                        // The sign-in box.
                        Rectangle {
                            implicitWidth: Kirigami.Units.gridUnit * 13
                            implicitHeight: box.implicitHeight + Kirigami.Units.largeSpacing * 2
                            radius: Kirigami.Units.cornerRadius * 2
                            color: Qt.rgba(preview.bg.r, preview.bg.g, preview.bg.b, 0.85)
                            border.width: 1
                            border.color: Qt.rgba(preview.fg.r, preview.fg.g, preview.fg.b, 0.15)
                            Layout.alignment: Qt.AlignHCenter

                            ColumnLayout {
                                id: box
                                anchors.fill: parent
                                anchors.margins: Kirigami.Units.largeSpacing
                                spacing: Kirigami.Units.smallSpacing

                                UqAvatar {
                                    visible: !page.greeter.hide_users
                                    name: page.me
                                    implicitWidth: Kirigami.Units.iconSizes.large
                                    implicitHeight: Kirigami.Units.iconSizes.large
                                    Layout.alignment: Qt.AlignHCenter
                                    Accessible.ignored: true
                                }
                                UqLabel {
                                    visible: !page.greeter.hide_users
                                    text: page.me !== "" ? page.me : "you"
                                    color: preview.fg
                                    font.bold: true
                                    Layout.alignment: Qt.AlignHCenter
                                    Accessible.ignored: true
                                }
                                // A typed user name instead of the list.
                                Rectangle {
                                    visible: !!page.greeter.hide_users
                                    Layout.fillWidth: true
                                    implicitHeight: Kirigami.Units.gridUnit * 1.6
                                    radius: Kirigami.Units.cornerRadius
                                    color: Qt.rgba(preview.fg.r, preview.fg.g, preview.fg.b, 0.08)
                                    border.width: 1
                                    border.color: Qt.rgba(preview.fg.r, preview.fg.g, preview.fg.b, 0.25)
                                    UqLabel {
                                        anchors.verticalCenter: parent.verticalCenter
                                        x: Kirigami.Units.smallSpacing
                                        text: "User name"
                                        color: preview.fg
                                        opacity: 0.6
                                        Accessible.ignored: true
                                    }
                                }
                                // The password box, with the accent as its focus line.
                                Rectangle {
                                    Layout.fillWidth: true
                                    implicitHeight: Kirigami.Units.gridUnit * 1.6
                                    radius: Kirigami.Units.cornerRadius
                                    color: Qt.rgba(preview.fg.r, preview.fg.g, preview.fg.b, 0.08)
                                    border.width: 1
                                    border.color: Kirigami.Theme.highlightColor
                                    UqLabel {
                                        anchors.verticalCenter: parent.verticalCenter
                                        x: Kirigami.Units.smallSpacing
                                        text: "••••••"
                                        color: preview.fg
                                        opacity: 0.6
                                        Accessible.ignored: true
                                    }
                                }
                            }
                        }
                    }
                }

                ColumnLayout {
                    spacing: Kirigami.Units.smallSpacing
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop

                    SettingRow {
                        settingId: "greeter-theme"
                        iconName: "preferences-desktop-color"
                        label: "Theme"
                        help: page.mine !== null && page.mine.available === false ? "Your own theme isn't available for the login screen; pick another." : ""
                        highlight: App.highlight
                        Layout.fillWidth: true

                        UqComboBox {
                            id: themeCombo
                            model: page.themeLabels
                            currentIndex: page.themeIndex()
                            enabled: page.idle
                            Accessible.name: "Login screen theme"
                            Layout.preferredWidth: Kirigami.Units.gridUnit * 18
                            Layout.maximumWidth: appearanceRow.width
                            onActivated: index => {
                                if (index === 0)
                                    page.saveLook("", "", !!page.greeter.hide_users, !!page.greeter.reduce_motion);
                                else {
                                    const v = page.variants[index - 1];
                                    page.saveLook(v.theme, v.variant, !!page.greeter.hide_users, !!page.greeter.reduce_motion);
                                }
                                themeCombo.currentIndex = Qt.binding(() => page.themeIndex());
                            }
                        }
                    }
                    SettingRow {
                        settingId: "greeter-wallpaper"
                        iconName: "preferences-desktop-wallpaper"
                        label: page.wallpaperSet ? "Picture" : "No picture"
                        help: "Pictures up to 20 MiB. The picture is copied, so later changes to your own wallpaper don't follow."
                        highlight: App.highlight
                        Layout.fillWidth: true

                        QQC2.Button {
                            objectName: "wallpaperMine"
                            text: "Use mine"
                            icon.name: "edit-copy"
                            enabled: page.idle && page.snap.wallpaper && page.snap.wallpaper.mine !== ""
                            QQC2.ToolTip.visible: hovered
                            QQC2.ToolTip.text: "Use your desktop's wallpaper"
                            onClicked: lm.wallpaperMine()
                        }
                        QQC2.Button {
                            objectName: "wallpaperPick"
                            text: "Choose…"
                            icon.name: "document-open"
                            enabled: page.idle
                            Accessible.name: "Choose a picture"
                            onClicked: pictureDialog.open()
                        }
                        QQC2.ToolButton {
                            objectName: "wallpaperClear"
                            text: "Clear"
                            icon.name: "edit-clear"
                            display: QQC2.AbstractButton.IconOnly
                            enabled: page.idle && page.wallpaperSet
                            QQC2.ToolTip.visible: hovered
                            QQC2.ToolTip.text: "No picture: the theme's background colour"
                            onClicked: lm.clearWallpaper()
                        }
                    }
                    SettingRow {
                        settingId: "greeter-displays"
                        iconName: "preferences-desktop-display"
                        label: "Display layout"
                        help: "Copying yours puts the login screen on the same screens, arranged as on your desktop."
                        highlight: App.highlight
                        Layout.fillWidth: true

                        QQC2.ButtonGroup {
                            id: displaysGroup
                        }
                        QQC2.Button {
                            objectName: "useMyDisplays"
                            text: "Mine"
                            icon.name: "preferences-desktop-display"
                            checkable: true
                            checked: !!(page.snap.monitors && page.snap.monitors.set)
                            enabled: page.idle && App.uqdUp
                            QQC2.ButtonGroup.group: displaysGroup
                            Accessible.name: "Use my display layout at login"
                            QQC2.ToolTip.visible: hovered
                            QQC2.ToolTip.text: "Copy your display layout now"
                            onClicked: {
                                lm.useMyDisplays();
                                checked = Qt.binding(() => !!(page.snap.monitors && page.snap.monitors.set));
                            }
                        }
                        QQC2.Button {
                            objectName: "automaticDisplays"
                            text: "Automatic"
                            icon.name: "view-refresh"
                            checkable: true
                            checked: !(page.snap.monitors && page.snap.monitors.set)
                            enabled: page.idle
                            QQC2.ButtonGroup.group: displaysGroup
                            Accessible.name: "Use automatic layout"
                            onClicked: {
                                if (page.snap.monitors && page.snap.monitors.set)
                                    lm.automaticDisplays();
                                checked = Qt.binding(() => !(page.snap.monitors && page.snap.monitors.set));
                            }
                        }
                    }
                }
            }
        }

        SettingsCard {
            title: "Privacy and motion"
            iconName: "view-hidden"
            help: "Changes here ask for an administrator password."

            SwitchRow {
                objectName: "hideUsers"
                settingId: "greeter-hide-users"
                iconName: "view-hidden"
                label: "Hide the list of users"
                help: "People type their user name instead."
                highlight: App.highlight
                value: !!page.greeter.hide_users
                enabled: page.idle
                Layout.fillWidth: true
                onChosen: on => page.saveLook(page.greeter.theme, page.greeter.variant, on, !!page.greeter.reduce_motion)
            }
            SwitchRow {
                objectName: "reduceMotion"
                settingId: "greeter-reduce-motion"
                iconName: "media-playback-paused"
                label: "Reduce motion"
                highlight: App.highlight
                value: !!page.greeter.reduce_motion
                enabled: page.idle
                Layout.fillWidth: true
                onChosen: on => page.saveLook(page.greeter.theme, page.greeter.variant, !!page.greeter.hide_users, on)
            }
        }

        SettingsCard {
            title: "Automatic login"
            iconName: "system-switch-user"

            SwitchRow {
                objectName: "autoLogin"
                settingId: "auto-login"
                iconName: "user-identity"
                label: page.me !== "" ? "Log in automatically as " + page.me : "Log in automatically"
                highlight: App.highlight
                value: page.me !== "" && page.autoUser === page.me
                enabled: page.idle && page.me !== ""
                Layout.fillWidth: true
                onChosen: on => lm.setAutoLogin(on)
            }
            RowLayout {
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Kirigami.Icon {
                    source: "dialog-warning"
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: Kirigami.Units.iconSizes.small
                    Accessible.ignored: true
                }
                UqLabel {
                    objectName: "autoLoginWarning"
                    text: "Anyone at this computer gets your session without a password. Use it only on an encrypted disk."
                    wrapMode: Text.Wrap
                    color: Kirigami.Theme.neutralTextColor
                    Layout.fillWidth: true
                }
            }
            UqLabel {
                objectName: "autoLoginOther"
                visible: page.autoUser !== "" && page.autoUser !== page.me
                text: "It's on for " + page.autoUser + ". Turning it on for you replaces that."
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
            }
        }
    }
}
