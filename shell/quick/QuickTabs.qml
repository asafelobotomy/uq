pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import "../core"
import "../logic/quickui.js" as U

// The quick-settings tab strip. Only the selected tab is in the Tab chain (roving focus): Left /
// Right (and Up / Down) select the previous / next tab and move focus to it, wrapping; Home / End
// jump to the ends. A click selects and focuses the tab. Every size, colour and radius comes from
// Theme.
Rectangle {
    id: root

    // [{key, label}] from quickui.shownTabs, and the key shown now.
    property var tabs: []
    property string current: ""
    signal chosen(string key)

    function tabFor(key) {
        for (var i = 0; i < repeater.count; i++) {
            var t = repeater.itemAt(i);
            if (t && t.key === key)
                return t;
        }
        return null;
    }

    // Focus the selected tab (on open, and after a tab switch from elsewhere in the panel).
    function focusCurrent() {
        var t = tabFor(root.current);
        if (t)
            t.forceActiveFocus();
    }

    // Focus moves first: the tab losing the selection leaves the Tab chain, which Qt refuses
    // while it holds focus.
    function select(key) {
        if (key === "")
            return;
        var t = tabFor(key);
        if (t)
            t.forceActiveFocus();
        root.chosen(key);
    }

    function go(step) {
        select(U.stepTab(root.tabs, root.current, step));
    }

    implicitHeight: row.implicitHeight + Theme.gapIn * 2
    radius: Theme.radius
    color: Theme.surfaceAlt
    Accessible.role: Accessible.PageTabList
    Accessible.name: "Quick settings sections"

    RowLayout {
        id: row
        anchors.fill: parent
        anchors.margins: Theme.gapIn
        spacing: Theme.gapIn

        Repeater {
            id: repeater
            model: root.tabs
            delegate: Rectangle {
                id: tab
                required property var modelData
                readonly property string key: modelData.key
                readonly property bool selected: root.current === key

                Layout.fillWidth: true
                // Equal widths: every tab asks for the same width and fills the rest.
                Layout.preferredWidth: Theme.gapIn
                implicitHeight: label.implicitHeight + Theme.gapIn * 2
                radius: Math.max(0, Theme.radius - Theme.gapIn)
                color: selected ? Theme.accent : (mouse.containsMouse ? Theme.overlay : "transparent")
                border.color: selected ? Theme.accentText : Theme.accent
                border.width: activeFocus ? Theme.borderWidth : 0
                activeFocusOnTab: selected
                Accessible.role: Accessible.PageTab
                Accessible.name: modelData.label
                Accessible.selectable: true
                Accessible.selected: selected
                Accessible.onPressAction: root.select(tab.key)

                Keys.onPressed: function (event) {
                    if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier))
                        return;
                    if (event.key === Qt.Key_Right || event.key === Qt.Key_Down)
                        root.go(1);
                    else if (event.key === Qt.Key_Left || event.key === Qt.Key_Up)
                        root.go(-1);
                    else if (event.key === Qt.Key_Home)
                        root.go("first");
                    else if (event.key === Qt.Key_End)
                        root.go("last");
                    else
                        return;
                    event.accepted = true;
                }

                UqText {
                    id: label
                    anchors.fill: parent
                    anchors.leftMargin: Theme.gapIn
                    anchors.rightMargin: Theme.gapIn
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    maximumLineCount: 1
                    text: tab.modelData.label
                    color: tab.selected ? Theme.accentText : Theme.text
                    font.bold: tab.selected
                }
                MouseArea {
                    id: mouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: root.select(tab.key)
                }
            }
        }
    }
}
