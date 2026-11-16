// A Settings page: its icon and title with the tab bar beside them (under them when the window
// is too narrow for both), any page-wide messages under that, then the current SettingsTab. The page remembers its tab between runs. A page with one tab
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

        // The title row: icon, title, tabs, then the page's own items. When the tabs don't fit
        // beside the title (a narrow window), they go on a row of their own under it, wrapping
        // onto more lines if they must; the title elides rather than push anything off.
        GridLayout {
            id: titleRow
            // Whether icon, title, tabs and the page's items all fit on one line.
            readonly property bool oneLine: {
                const sp = Kirigami.Units.largeSpacing;
                const need = (pageIcon.visible ? pageIcon.implicitWidth + sp : 0)
                    + pageTitle.implicitWidth
                    + (bar.visible ? bar.oneLineWidth + sp * 2 : 0)
                    + (headerExtra.implicitWidth > 0 ? headerExtra.implicitWidth + sp : 0);
                return need <= titleRow.width;
            }
            columns: oneLine ? 5 : 3
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
            Layout.rightMargin: Kirigami.Units.largeSpacing * 2
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.bottomMargin: Kirigami.Units.smallSpacing

            Kirigami.Icon {
                id: pageIcon
                visible: page.iconName !== ""
                source: page.iconName
                implicitWidth: Kirigami.Units.iconSizes.medium
                implicitHeight: Kirigami.Units.iconSizes.medium
                Layout.row: 0
                Layout.column: 0
                Accessible.ignored: true
            }
            UqLabel {
                id: pageTitle
                objectName: "pageTitle"
                text: page.title
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.4
                font.bold: true
                elide: Text.ElideRight
                Layout.row: 0
                Layout.column: 1
                Layout.fillWidth: !titleRow.oneLine
                Layout.minimumWidth: Math.min(implicitWidth, Kirigami.Units.gridUnit * 4)
                Accessible.role: Accessible.Heading
            }
            // The tabs: a row of buttons sized to their own icon and name. (QQC2.TabBar
            // squeezed its buttons over each other when they carried their own contentItem.)
            Flow {
                id: bar
                objectName: "pageTabs"
                // The width of every tab side by side.
                readonly property real oneLineWidth: {
                    let w = 0;
                    for (let i = 0; i < tabRepeater.count; i++) {
                        const b = tabRepeater.itemAt(i);
                        if (b)
                            w += b.implicitWidth + (w > 0 ? bar.spacing : 0);
                    }
                    return w;
                }
                visible: page.shownTabs.length > 1
                spacing: Kirigami.Units.smallSpacing
                Layout.row: titleRow.oneLine ? 0 : 1
                Layout.column: titleRow.oneLine ? 2 : 0
                Layout.columnSpan: titleRow.oneLine ? 1 : 3
                Layout.leftMargin: titleRow.oneLine ? Kirigami.Units.largeSpacing : 0
                Layout.fillWidth: !titleRow.oneLine
                Layout.preferredWidth: titleRow.oneLine ? oneLineWidth : -1
                Layout.preferredHeight: implicitHeight
                Accessible.role: Accessible.PageTabList
                Accessible.name: page.title + " tabs"

                Repeater {
                    id: tabRepeater
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
                visible: titleRow.oneLine
                Layout.row: 0
                Layout.column: 3
                Layout.fillWidth: true
            }
            RowLayout {
                id: headerExtra
                spacing: Kirigami.Units.largeSpacing
                Layout.row: 0
                Layout.column: titleRow.oneLine ? 4 : 2
            }
        }
        ColumnLayout {
            id: bannerColumn
            spacing: Kirigami.Units.smallSpacing
            // Never more than its banners: with every banner hidden, spare height (a tab whose
            // content arrived late) went here and left a gap under the title.
            Layout.maximumHeight: implicitHeight
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
