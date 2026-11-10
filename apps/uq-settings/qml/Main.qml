// The Settings window: the category sidebar on the left, the current page on the right.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings

Kirigami.ApplicationWindow {
    id: root

    readonly property var pageFiles: ({
        appearance: "pages/AppearancePage.qml",
        wallpaper: "pages/WallpaperPage.qml"
    })

    function showCurrent() {
        const file = pageFiles[App.page];
        if (file)
            stack.replace(null, Qt.resolvedUrl(file));
    }

    title: "uQ Settings"
    visible: true
    width: Kirigami.Units.gridUnit * 50
    height: Kirigami.Units.gridUnit * 34
    minimumWidth: Kirigami.Units.gridUnit * 30
    minimumHeight: Kirigami.Units.gridUnit * 20

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
                pages: JSON.parse(App.pagesJson || "[]")
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
        readonly property var ids: JSON.parse(App.pagesJson || "[]").map(p => p.id)

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
