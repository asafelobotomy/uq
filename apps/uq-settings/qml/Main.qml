// The Settings window: the category sidebar on the left, the current page on the right.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings

Kirigami.ApplicationWindow {
    id: root

    readonly property var pages: JSON.parse(App.pagesJson || "[]")

    function showCurrent() {
        const page = root.pages.find(p => p.id === App.page);
        if (!page)
            return;
        // A plugin page is loaded by PluginPage, which turns a QML error into a card.
        if (page.plugin)
            stack.replace(null, Qt.resolvedUrl("PluginPage.qml"), { source: page.qml, theme: JSON.parse(App.themeJson || "{}"), highlight: Qt.binding(() => App.highlight) });
        else
            stack.replace(null, Qt.resolvedUrl(page.qml));
    }

    // Coming back to the window re-reads the page: user.lua or the live session may have changed
    // meanwhile, and the rows' sources should say so. Pages that read from uqd have `refresh()`.
    onActiveChanged: {
        const page = stack.currentItem;
        if (root.active && page && typeof page.refresh === "function")
            page.refresh();
    }

    title: "uQ Settings"
    visible: true
    width: Kirigami.Units.gridUnit * 50
    height: Kirigami.Units.gridUnit * 34
    minimumWidth: Kirigami.Units.gridUnit * 30
    minimumHeight: Kirigami.Units.gridUnit * 20

    header: QQC2.ToolBar {
        RowLayout {
            anchors.fill: parent
            Item {
                Layout.fillWidth: true
            }
            Search {
                id: search
                searchFunction: query => App.search(query)
                onActivated: (pageId, settingId) => App.showPage(pageId, settingId)
                Layout.preferredWidth: Kirigami.Units.gridUnit * 18
            }
        }
    }

    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None
    pageStack.initialPage: Kirigami.Page {
        padding: 0
        globalToolBarStyle: Kirigami.ApplicationHeaderStyle.None

        RowLayout {
            anchors.fill: parent
            spacing: 0

            Sidebar {
                id: sidebar
                Layout.fillHeight: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 12
                pages: root.pages
                current: App.page
                focus: true
                onChosen: pageId => App.showPage(pageId, "")
            }
            Kirigami.Separator {
                Layout.fillHeight: true
            }
            QQC2.StackView {
                id: stack
                Layout.fillWidth: true
                Layout.fillHeight: true
                activeFocusOnTab: true
                Accessible.role: Accessible.Pane
                Accessible.name: "Page"
                Component.onCompleted: root.showCurrent()
            }
        }
    }

    Connections {
        target: App
        function onPageChanged() {
            root.showCurrent();
        }
        function onRaiseRequested() {
            root.show();
            root.raise();
            root.requestActivate();
        }
    }

    // `--screenshots <dir>`: show each page, save it as <dir>/<page>.png, then quit.
    Timer {
        id: shots

        property int step: 0
        readonly property var ids: root.pages.map(p => p.id)

        interval: 600
        repeat: true
        running: App.screenshotDir !== ""
        onTriggered: {
            if (step > 0 && !App.grabWindow(App.screenshotDir + "/" + ids[step - 1] + ".png"))
                console.warn("uq-settings: couldn't save " + ids[step - 1] + ".png");
            if (step === ids.length) {
                Qt.quit();
                return;
            }
            App.showPage(ids[step], "");
            step += 1;
        }
    }
}
