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
    readonly property int columns: tab.width >= Kirigami.Units.gridUnit * 61 ? 2 : 1
    // The width the cards are laid out in: the tab's own width less a fixed allowance for the
    // scroll bar. Not availableWidth, which shrinks when the scroll bar appears: a page about one
    // screen tall then re-flowed, lost its scroll bar, re-flowed back, and never settled.
    readonly property real layoutWidth: Math.max(0, tab.width - Kirigami.Units.gridUnit)
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
    contentHeight: grid.height + Kirigami.Units.largeSpacing * 4
    Accessible.role: Accessible.PageTab
    Accessible.name: tab.title

    GridLayout {
        id: grid
        // The same side margin as the page's title row, so cards line up under the title.
        width: Math.min(tab.layoutWidth - Kirigami.Units.largeSpacing * 4, Kirigami.Units.gridUnit * 96)
        // Centred in the whole tab, so the margins either side match (the scroll bar, when
        // there is one, sits in the right margin).
        x: Math.max(Kirigami.Units.largeSpacing * 2, (tab.width - width) / 2)
        y: Kirigami.Units.largeSpacing * 2
        columns: tab.columns
        columnSpacing: Kirigami.Units.largeSpacing * 2
        rowSpacing: Kirigami.Units.largeSpacing * 2
        // Equal columns: each card fills its share.
        uniformCellWidths: true
    }
}
