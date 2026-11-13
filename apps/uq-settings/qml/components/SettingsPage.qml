// A Settings page: its icon and title with the tab bar beside them, any page-wide messages under
// that, then the current SettingsTab. The page remembers its tab between runs. A page with one tab
// shows no tab bar. Ctrl+Tab and Ctrl+Shift+Tab (or Ctrl+PgDown and Ctrl+PgUp) change tabs.
import QtCore as QtCore
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings

FocusScope {
    id: page

    /// The page's id, for remembering its tab (`App.page` when empty).
    property string pageId: ""
    /// The page's sidebar entry: the title and icon when the page names none.
    readonly property var entry: {
        try {
            return JSON.parse(App.pagesJson || "[]").find(p => p.id === page.key) || ({});
        } catch (e) {
            return {};
        }
    }
    property string title: page.entry.name || ""
    property string iconName: page.entry.icon || ""
    /// Items on the title row, after the tabs (a live clock, a status, a main switch).
    property alias headerItems: headerExtra.data
    /// Page-wide messages (errors, "not available"), between the header and the tabs.
    property alias banners: bannerColumn.data
    default property alias tabs: stack.data
    readonly property alias currentIndex: stack.currentIndex
    readonly property var tabList: {
        const out = [];
        for (let i = 0; i < stack.children.length; i++) {
            const c = stack.children[i];
            if (c && c.isSettingsTab === true)
                out.push(c);
        }
        return out;
    }
    readonly property var shownTabs: page.tabList.filter(t => t.available)
    readonly property string key: page.pageId !== "" ? page.pageId : App.page

    function show(tab) {
        const i = page.tabList.indexOf(tab);
        if (i < 0)
            return;
        stack.currentIndex = i;
        if (App.screenshotDir === "")
            store.setValue(page.key, tab.title);
    }
    function step(by) {
        const shown = page.shownTabs;
        if (shown.length < 2)
            return;
        const at = shown.indexOf(page.tabList[stack.currentIndex]);
        page.show(shown[(at + by + shown.length) % shown.length]);
    }
    /// Show the remembered tab, or the first one that is available.
    function restore() {
        for (const t of page.tabList)
            t.activateHook = page.show;
        const want = App.screenshotDir === "" ? String(store.value(page.key, "")) : "";
        const remembered = page.shownTabs.find(t => t.title === want);
        const first = remembered || page.shownTabs[0];
        if (first)
            stack.currentIndex = page.tabList.indexOf(first);
    }

    Accessible.role: Accessible.Pane
    Accessible.name: page.title

    Component.onCompleted: Qt.callLater(page.restore)
    // A tab that stops being available while shown (the touchpad was unplugged).
    onShownTabsChanged: {
        const cur = page.tabList[stack.currentIndex];
        if (cur && !cur.available && page.shownTabs.length > 0)
            stack.currentIndex = page.tabList.indexOf(page.shownTabs[0]);
    }

    QtCore.Settings {
        id: store
        location: QtCore.StandardPaths.writableLocation(QtCore.StandardPaths.GenericStateLocation) + "/uq/settings-app.conf"
        category: "tabs"
    }

    Shortcut {
        sequences: ["Ctrl+Tab", "Ctrl+PgDown"]
        enabled: page.visible
        onActivated: page.step(1)
    }
    Shortcut {
        sequences: ["Ctrl+Shift+Tab", "Ctrl+Backtab", "Ctrl+PgUp"]
        enabled: page.visible
        onActivated: page.step(-1)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // The title row: icon, title, tabs, then the page's own items.
        RowLayout {
            spacing: Kirigami.Units.largeSpacing
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
            Layout.rightMargin: Kirigami.Units.largeSpacing * 2
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.bottomMargin: Kirigami.Units.smallSpacing

            Kirigami.Icon {
                visible: page.iconName !== ""
                source: page.iconName
                implicitWidth: Kirigami.Units.iconSizes.medium
                implicitHeight: Kirigami.Units.iconSizes.medium
                Accessible.ignored: true
            }
            UqLabel {
                objectName: "pageTitle"
                text: page.title
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.4
                font.bold: true
                Accessible.role: Accessible.Heading
            }
            // The tabs: a row of buttons sized to their own icon and name. (QQC2.TabBar
            // squeezed its buttons over each other when they carried their own contentItem.)
            RowLayout {
                id: bar
                objectName: "pageTabs"
                visible: page.shownTabs.length > 1
                spacing: Kirigami.Units.smallSpacing
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Accessible.role: Accessible.PageTabList
                Accessible.name: page.title + " tabs"

                Repeater {
                    model: page.shownTabs
                    QQC2.ToolButton {
                        id: tabButton
                        required property var modelData
                        readonly property bool current: page.tabList[stack.currentIndex] === modelData
                        objectName: "tab-" + modelData.title
                        checkable: true
                        checked: tabButton.current
                        Accessible.role: Accessible.PageTab
                        Accessible.name: modelData.title
                        leftPadding: Kirigami.Units.largeSpacing
                        rightPadding: Kirigami.Units.largeSpacing
                        onClicked: page.show(modelData)
                        Keys.onLeftPressed: page.step(-1)
                        Keys.onRightPressed: page.step(1)
                        contentItem: RowLayout {
                            spacing: Kirigami.Units.smallSpacing
                            Kirigami.Icon {
                                visible: tabButton.modelData.iconName !== ""
                                source: tabButton.modelData.iconName
                                implicitWidth: Kirigami.Units.iconSizes.small
                                implicitHeight: Kirigami.Units.iconSizes.small
                                Accessible.ignored: true
                            }
                            UqLabel {
                                text: tabButton.modelData.title
                                font.bold: tabButton.current
                            }
                        }
                        // The current tab's underline.
                        Rectangle {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            height: 2
                            visible: tabButton.current
                            color: Kirigami.Theme.highlightColor
                        }
                    }
                }
            }
            Item {
                Layout.fillWidth: true
            }
            RowLayout {
                id: headerExtra
                spacing: Kirigami.Units.largeSpacing
            }
        }
        ColumnLayout {
            id: bannerColumn
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
            Layout.rightMargin: Kirigami.Units.largeSpacing * 2
        }
        Kirigami.Separator {
            Layout.fillWidth: true
        }
        StackLayout {
            id: stack
            Layout.fillWidth: true
            Layout.fillHeight: true
        }
    }
}
