// Loads a plugin's page file, only when this item is created (the page is shown). A QML error
// in the plugin shows an ErrorCard instead of taking the app down. Free of the `App` singleton:
// everything comes in as properties. The page's root item gets `ctx` (PluginContext: theme,
// command, dbus) and, as in Phase 3, `theme` and `highlight`, each if it declares them.
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "components"

Item {
    id: page

    /// The plugin's page file (absolute file URL).
    property url source
    /// The design tokens handed to the plugin page (a `theme` property on its root item, if it
    /// declares one).
    property var theme: ({})
    /// The row to flash, for pages that have `SettingRow`s.
    property string highlight
    /// The page's id (page.toml), which its commands and D-Bus requests are checked against.
    property string pageId
    /// The PluginHost singleton; null (as in tests) makes every request fail with an error.
    property var host: null
    readonly property alias context: context
    readonly property bool failed: loader.status === Loader.Error
    readonly property string errorText: failed ? describe() : ""

    function describe() {
        const component = loader.sourceComponent;
        const text = component ? component.errorString() : "";
        return text !== "" ? text : "The page's QML file couldn't be loaded: " + source;
    }

    Component.onCompleted: loader.setSource(source)

    Loader {
        id: loader
        anchors.fill: parent
        asynchronous: false
        visible: status === Loader.Ready
        onLoaded: {
            // Bound, so a theme change while the page is open reaches it.
            if (item && "theme" in item)
                item.theme = Qt.binding(() => page.theme);
            if (item && "highlight" in item)
                item.highlight = Qt.binding(() => page.highlight);
            if (item && "ctx" in item)
                item.ctx = Qt.binding(() => context.ctx);
        }
    }

    PluginContext {
        id: context
        host: page.host
        pageId: page.pageId
        theme: page.theme
        onConfirmNeeded: (request, text) => confirmDialog.ask(request, text)
    }

    CommandConfirmDialog {
        id: confirmDialog
        objectName: "commandConfirm"
        onAnswered: (request, confirmed) => context.answer(request, confirmed)
    }

    ErrorCard {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: Kirigami.Units.largeSpacing
        visible: page.failed
        title: "This page couldn't load"
        detail: page.errorText
    }
}
