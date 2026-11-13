// One tab of a SettingsPage: its own scroll area, laying its SettingsCards out in two columns
// when the window is wide enough (one at narrow widths), so a 1920x1080 window isn't one thin
// column with empty space either side.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

QQC2.ScrollView {
    id: tab

    property string title: ""
    property string iconName: ""
    /// Hide the tab's button (a tab with nothing to show on this computer).
    property bool available: true
    /// Columns at this width: two from about 1100 px.
    readonly property int columns: tab.availableWidth >= Kirigami.Units.gridUnit * 60 ? 2 : 1
    /// Marks this as a tab for SettingRow's search highlight (see activate).
    readonly property bool isSettingsTab: true
    /// Set by SettingsPage: show this tab.
    property var activateHook: null
    default property alias content: grid.data

    /// Show this tab (a search result inside it).
    function activate() {
        if (tab.activateHook)
            tab.activateHook(tab);
    }

    contentWidth: availableWidth
    contentHeight: grid.height + Kirigami.Units.largeSpacing * 2
    Accessible.role: Accessible.PageTab
    Accessible.name: tab.title

    GridLayout {
        id: grid
        width: Math.min(tab.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 96)
        x: Math.max(Kirigami.Units.largeSpacing, (tab.availableWidth - width) / 2)
        y: Kirigami.Units.largeSpacing
        columns: tab.columns
        columnSpacing: Kirigami.Units.largeSpacing
        rowSpacing: Kirigami.Units.largeSpacing
        // Equal columns: each card fills its share.
        uniformCellWidths: true
    }
}
