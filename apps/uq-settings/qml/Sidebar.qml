// The page list, grouped under category headings. A category with no pages has no heading,
// since headings come from the pages themselves. Arrow keys move, Enter or Space opens.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "components"

QQC2.ScrollView {
    id: sidebar

    /// `[{id, name, icon, category, categoryName}]`, already in category order.
    property var pages: []
    /// The id of the page on show.
    property string current
    readonly property ListModel pageModel: ListModel {}
    signal chosen(string pageId)

    function indexOf(id) {
        for (let i = 0; i < pageModel.count; ++i) {
            if (pageModel.get(i).pageId === id)
                return i;
        }
        return -1;
    }

    onPagesChanged: {
        pageModel.clear();
        for (const p of pages)
            pageModel.append({ pageId: p.id, name: p.name, iconName: p.icon, categoryName: p.categoryName });
        list.currentIndex = indexOf(current);
    }
    onCurrentChanged: list.currentIndex = indexOf(current)

    function openCurrent() {
        if (list.currentIndex >= 0)
            chosen(pageModel.get(list.currentIndex).pageId);
    }

    ListView {
        id: list

        model: pageModel
        focus: true
        activeFocusOnTab: true
        keyNavigationEnabled: true
        clip: true
        Accessible.role: Accessible.List
        Accessible.name: "Settings pages"

        Keys.onReturnPressed: sidebar.openCurrent()
        Keys.onEnterPressed: sidebar.openCurrent()
        Keys.onSpacePressed: sidebar.openCurrent()

        section.property: "categoryName"
        section.delegate: UqLabel {
            required property string section
            width: ListView.view.width
            topPadding: Kirigami.Units.largeSpacing
            bottomPadding: Kirigami.Units.smallSpacing
            leftPadding: Kirigami.Units.largeSpacing
            text: section
            font.bold: true
            opacity: 0.7
            Accessible.role: Accessible.Heading
        }

        delegate: QQC2.ItemDelegate {
            id: item
            required property int index
            required property string pageId
            required property string name
            required property string iconName

            width: ListView.view.width
            highlighted: ListView.isCurrentItem
            focusPolicy: Qt.NoFocus
            Accessible.role: Accessible.ListItem
            Accessible.name: name
            Accessible.selected: pageId === sidebar.current
            onClicked: {
                list.currentIndex = index;
                sidebar.chosen(pageId);
            }

            contentItem: RowLayout {
                spacing: Kirigami.Units.smallSpacing
                Kirigami.Icon {
                    source: item.iconName
                    implicitWidth: Kirigami.Units.iconSizes.smallMedium
                    implicitHeight: Kirigami.Units.iconSizes.smallMedium
                    Accessible.ignored: true
                }
                UqLabel {
                    text: item.name
                    elide: Text.ElideRight
                    highlighted: item.highlighted || item.down
                    Layout.fillWidth: true
                    Accessible.ignored: true
                }
            }
        }
    }
}
