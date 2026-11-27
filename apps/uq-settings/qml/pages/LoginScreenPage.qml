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
import "loginscreen"

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

    LoginTab {
        lm: lm
        page: page
        pictureDialog: pictureDialog
    }
}
