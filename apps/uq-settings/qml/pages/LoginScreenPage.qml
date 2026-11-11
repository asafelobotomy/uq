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

QQC2.ScrollView {
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
    function themeIndex() {
        const i = page.variants.findIndex(v => v.theme === page.greeter.theme && v.variant === page.greeter.variant);
        return i < 0 ? -1 : i + 1;
    }
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

    Accessible.role: Accessible.Pane
    Accessible.name: "Login screen"
    contentWidth: availableWidth

    LoginScreenModel {
        id: lm
        onStateJsonChanged: page.showNews()
    }

    FileDialog {
        id: pictureDialog
        title: "Choose a picture for the login screen"
        nameFilters: ["Pictures (*.png *.jpg *.jpeg *.webp)"]
        onAccepted: lm.wallpaperFile(String(selectedFile))
    }

    ColumnLayout {
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 44)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Login screen"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        UqLabel {
            text: "How the screen where you log in looks. Changes need an administrator password."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "helperMissing"
            visible: page.loaded && !page.canSave
            title: "These settings can only be viewed here"
            detail: String(page.snap.helper_note || "")
            Layout.fillWidth: true
        }
        RuleMessage {
            objectName: "loginError"
            negative: true
            closable: false
            lines: lm.error !== "" ? ["That wasn't saved: " + lm.error] : []
            Layout.fillWidth: true
        }
        RuleMessage {
            id: noticeMessage
            objectName: "loginNotice"
            Layout.fillWidth: true
        }
        QQC2.BusyIndicator {
            visible: lm.busy
            running: lm.busy
            Accessible.name: "Waiting for the password prompt"
        }

        SettingRow {
            settingId: "greeter-theme"
            label: "Theme"
            highlight: App.highlight
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            UqComboBox {
                id: themeCombo
                model: page.themeLabels
                currentIndex: page.themeIndex()
                enabled: page.idle
                implicitContentWidthPolicy: QQC2.ComboBox.WidestText
                Accessible.name: "Login screen theme"
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
        UqLabel {
            visible: page.mine !== null && page.mine.available === false
            text: "Your own theme isn't available for the login screen; pick another."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }

        SettingRow {
            settingId: "greeter-wallpaper"
            label: page.snap.wallpaper && page.snap.wallpaper.set ? "Wallpaper: a picture is set" : "Wallpaper: none, the theme's background colour"
            highlight: App.highlight
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "wallpaperMine"
                text: "Use mine"
                icon.name: "edit-copy"
                enabled: page.idle && page.snap.wallpaper && page.snap.wallpaper.mine !== ""
                onClicked: lm.wallpaperMine()
            }
            QQC2.Button {
                objectName: "wallpaperPick"
                text: "Choose a picture…"
                icon.name: "document-open"
                enabled: page.idle
                onClicked: pictureDialog.open()
            }
            QQC2.Button {
                objectName: "wallpaperClear"
                text: "Clear"
                icon.name: "edit-clear"
                enabled: page.idle && page.snap.wallpaper && page.snap.wallpaper.set
                onClicked: lm.clearWallpaper()
            }
        }
        UqLabel {
            text: "Pictures up to 20 MiB. The picture is copied, so later changes to your own wallpaper don't follow."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }

        SettingRow {
            settingId: "greeter-displays"
            label: page.snap.monitors && page.snap.monitors.set ? "Display layout: copied from yours" : "Display layout: arranged automatically"
            highlight: App.highlight
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "useMyDisplays"
                text: "Use my display layout at login"
                icon.name: "preferences-desktop-display"
                enabled: page.idle && App.uqdUp
                onClicked: lm.useMyDisplays()
            }
        }

        SwitchRow {
            objectName: "hideUsers"
            settingId: "greeter-hide-users"
            label: "Hide the list of users (type a user name instead)"
            highlight: App.highlight
            value: !!page.greeter.hide_users
            enabled: page.idle
            Layout.fillWidth: true
            onChosen: on => page.saveLook(page.greeter.theme, page.greeter.variant, on, !!page.greeter.reduce_motion)
        }
        SwitchRow {
            objectName: "reduceMotion"
            settingId: "greeter-reduce-motion"
            label: "Reduce motion"
            highlight: App.highlight
            value: !!page.greeter.reduce_motion
            enabled: page.idle
            Layout.fillWidth: true
            onChosen: on => page.saveLook(page.greeter.theme, page.greeter.variant, !!page.greeter.hide_users, on)
        }

        Kirigami.Separator {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SwitchRow {
            objectName: "autoLogin"
            settingId: "auto-login"
            label: page.me !== "" ? "Log in automatically as " + page.me : "Log in automatically"
            highlight: App.highlight
            value: page.me !== "" && page.autoUser === page.me
            enabled: page.idle && page.me !== ""
            Layout.fillWidth: true
            onChosen: on => lm.setAutoLogin(on)
        }
        UqLabel {
            objectName: "autoLoginWarning"
            text: "Anyone at this computer gets your session without a password. Only turn this on if the whole disk is encrypted."
            wrapMode: Text.Wrap
            color: Kirigami.Theme.neutralTextColor
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            objectName: "autoLoginOther"
            visible: page.autoUser !== "" && page.autoUser !== page.me
            text: "Auto-login is on for " + page.autoUser + ". Turning it on for you replaces that."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
