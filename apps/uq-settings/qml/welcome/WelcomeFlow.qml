// Welcome (Phase 5 spec §5.1): a few steps that set up the desktop the first time. Each step
// can be skipped, and every change applies at once through the same models as the full
// Settings pages. Welcome counts as done as soon as it shows, so closing the window at any
// step is fine: nothing asks again.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"

Item {
    id: flow

    /// Finish was pressed on the last step.
    signal finished()

    readonly property var snap: {
        try {
            return JSON.parse(wm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var state: snap.flow || null
    readonly property string current: state ? state.current : ""
    readonly property var steps: state ? state.steps : []
    readonly property string title: {
        const s = flow.steps.find(x => x.id === flow.current);
        return s ? s.title : "Welcome to uQ";
    }
    /// Step ids and the files that show them. Wallpaper and Displays are the full pages.
    readonly property var sources: ({
        hello: "HelloStep.qml",
        look: "LookStep.qml",
        wallpaper: "../pages/WallpaperPage.qml",
        layouts: "LayoutsStep.qml",
        displays: "../pages/DisplaysPage.qml",
        apps: "AppsStep.qml",
        fingerprint: "FingerprintStep.qml",
        done: "DoneStep.qml"
    })

    Accessible.role: Accessible.Pane
    Accessible.name: "Welcome"

    WelcomeModel {
        id: wm
    }

    Component.onCompleted: wm.start(App.welcomeFull)

    Connections {
        target: App
        // uqd came up after Welcome opened: mark it done now.
        function onUqdUpChanged() {
            if (App.uqdUp)
                wm.start(App.welcomeFull);
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing * 2
        spacing: Kirigami.Units.largeSpacing

        UqLabel {
            objectName: "welcomeTitle"
            text: flow.title
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.8
            font.bold: true
            Accessible.role: Accessible.Heading
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "welcomeError"
            visible: wm.error !== ""
            title: "That didn't work"
            detail: wm.error
            Layout.fillWidth: true
        }
        Loader {
            id: stepLoader
            objectName: "stepLoader"
            active: flow.current !== ""
            source: flow.sources[flow.current] || ""
            Layout.fillWidth: true
            Layout.fillHeight: true
            // Welcome's own steps read its state and call its model; the full pages don't.
            onLoaded: {
                if (item && "welcome" in item) {
                    item.welcome = Qt.binding(() => flow.snap);
                    item.model = wm;
                }
            }
        }
        WelcomeNav {
            objectName: "welcomeNav"
            index: flow.state ? flow.state.index : 0
            count: flow.state ? flow.state.count : 1
            busy: wm.busy
            Layout.fillWidth: true
            onBack: wm.back()
            onSkip: wm.skip()
            onNext: wm.next()
            onFinish: flow.finished()
        }
    }
}
