// Global search: a field with a results popup. Up/Down move, Enter opens the result, Esc closes
// the popup (a second Esc clears the field). An empty query shows nothing.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "components"

Kirigami.SearchField {
    id: field

    /// `[{pageId, settingId, label, pageName}]` for the current text.
    property var hits: []
    readonly property bool open: popup.visible

    function refresh() {
        const q = text.trim();
        hits = q === "" ? [] : JSON.parse(App.search(q) || "[]");
        results.currentIndex = hits.length > 0 ? 0 : -1;
        if (q === "")
            popup.close();
        else
            popup.open();
    }
    function choose(index) {
        if (index < 0 || index >= hits.length)
            return;
        const hit = hits[index];
        popup.close();
        clear();
        App.showPage(hit.pageId, hit.settingId);
    }

    placeholderText: "Search settings"
    Accessible.name: "Search settings"
    onTextChanged: refresh()
    onAccepted: choose(results.currentIndex)

    Keys.onDownPressed: if (hits.length > 0) results.currentIndex = Math.min(results.currentIndex + 1, hits.length - 1)
    Keys.onUpPressed: if (hits.length > 0) results.currentIndex = Math.max(results.currentIndex - 1, 0)
    Keys.onEscapePressed: event => {
        if (popup.visible) {
            popup.close();
        } else {
            clear();
        }
    }

    QQC2.Popup {
        id: popup

        y: field.height + Kirigami.Units.smallSpacing
        width: Math.max(field.width, Kirigami.Units.gridUnit * 20)
        height: Math.min(results.contentHeight, Kirigami.Units.gridUnit * 20) + topPadding + bottomPadding
        padding: Kirigami.Units.smallSpacing
        focus: false
        closePolicy: QQC2.Popup.CloseOnEscape | QQC2.Popup.CloseOnPressOutsideParent

        contentItem: ListView {
            id: results

            model: field.hits
            clip: true
            currentIndex: -1
            Accessible.role: Accessible.List
            Accessible.name: "Search results"

            delegate: QQC2.ItemDelegate {
                required property int index
                required property var modelData

                width: ListView.view.width
                highlighted: ListView.isCurrentItem
                focusPolicy: Qt.NoFocus
                Accessible.role: Accessible.ListItem
                Accessible.name: modelData.label + ", " + modelData.pageName
                onClicked: field.choose(index)

                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing
                    UqLabel {
                        text: modelData.label
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                        Accessible.ignored: true
                    }
                    UqLabel {
                        text: modelData.pageName
                        opacity: 0.7
                        Accessible.ignored: true
                    }
                }
            }

            UqLabel {
                anchors.centerIn: parent
                visible: field.hits.length === 0
                text: "No matching settings"
                opacity: 0.7
            }
        }
    }
}
