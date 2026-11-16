// One tab of a SettingsPage: its own scroll area, laying its SettingsCards out in a centred block
// of at most `columnWidth` per column: one column, and two once the window has room for two
// full-size ones. Cards never stretch past that, so a label and its control stay near each other
// on a 1920x1080 window (or wider) instead of sitting at opposite edges.
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
    /// The widest a column of cards gets (the shared rule: SettingRow caps itself at the same width).
    readonly property real columnWidth: Kirigami.Units.gridUnit * 44
    /// The room either side of the cards, and between two columns.
    readonly property real sideMargin: Kirigami.Units.largeSpacing * 2
    readonly property real columnGap: Kirigami.Units.largeSpacing * 2
    /// Columns at this width: two once each can be at least 34 grid units wide (about 1400 px in
    /// all), otherwise one.
    readonly property int columns: tab.layoutWidth - tab.sideMargin * 2 >= Kirigami.Units.gridUnit * 34 * 2 + tab.columnGap ? 2 : 1
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
        // At most `columnWidth` per column; narrower windows shrink the cards instead.
        width: Math.min(tab.layoutWidth - tab.sideMargin * 2, tab.columns * tab.columnWidth + (tab.columns - 1) * tab.columnGap)
        // Centred in the whole tab, so the margins either side match (the scroll bar, when
        // there is one, sits in the right margin).
        x: Math.max(tab.sideMargin, (tab.width - width) / 2)
        y: Kirigami.Units.largeSpacing * 2
        columns: tab.columns
        columnSpacing: tab.columnGap
        rowSpacing: Kirigami.Units.largeSpacing * 2
        // Equal columns: each card fills its share.
        uniformCellWidths: true
    }
}
