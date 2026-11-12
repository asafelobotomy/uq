// Global search: a field with a results popup. Up/Down move, Enter opens the result, Esc closes
// the popup (a second Esc clears the field). An empty query shows nothing.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "components"

Kirigami.SearchField {
    id: field

    /// `function(query) -> json string` of `[{pageId, settingId, label, pageName}]`.
    property var searchFunction: query => "[]"
    /// The hits for the current text.
    property var hits: []
    readonly property bool open: popup.visible
    property alias resultsPopup: popup
    property alias emptyLabel: emptyLabel
    signal activated(string pageId, string settingId)

    function refresh() {
        if (text.length > 64) {
            text = text.slice(0, 64); // the field has no maxLength; re-enters refresh()
            return;
        }
        const q = text.trim();
        hits = q === "" ? [] : JSON.parse(searchFunction(q) || "[]");
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
        activated(hit.pageId, hit.settingId);
    }

    autoAccept: false
    placeholderText: "Search settings"
    Accessible.name: "Search settings"
    onTextChanged: refresh()
    onAccepted: choose(results.currentIndex)

    Keys.onDownPressed: {
        if (!popup.visible && text.trim() !== "")
            popup.open();
        else if (hits.length > 0)
            results.currentIndex = Math.min(results.currentIndex + 1, hits.length - 1);
    }
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
        height: Math.max(Math.min(results.contentHeight, Kirigami.Units.gridUnit * 20), field.hits.length === 0 ? Kirigami.Units.gridUnit * 2 : 0) + topPadding + bottomPadding
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
                id: hit
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
                        highlighted: hit.highlighted || hit.down
                        Layout.fillWidth: true
                        Accessible.ignored: true
                    }
                    UqLabel {
                        text: modelData.pageName
                        opacity: 0.7
                        highlighted: hit.highlighted || hit.down
                        Accessible.ignored: true
                    }
                }
            }

            UqLabel {
                id: emptyLabel
                anchors.centerIn: parent
                visible: field.hits.length === 0
                text: "No matching settings"
                opacity: 0.7
            }
        }
    }
}
