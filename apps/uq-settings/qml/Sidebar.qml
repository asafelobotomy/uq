// The page list, grouped under category headings. A category with no pages has no heading,
// since headings come from the pages themselves. Arrow keys move, Enter or Space opens. In a
// narrow window (`compact`) it is a column of icons: each page's name in its tooltip, and a thin
// rule between categories in place of their headings.
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
    /// Icons only, for a narrow window.
    property bool compact: false
    /// The width it wants: room for the names, or for an icon.
    readonly property real wantedWidth: compact
        ? Kirigami.Units.iconSizes.smallMedium + Kirigami.Units.largeSpacing * 4
        : Kirigami.Units.gridUnit * 12

    // Compact: no scroll bar eating the narrow column (the wheel and the keys still scroll).
    QQC2.ScrollBar.vertical.policy: compact ? QQC2.ScrollBar.AlwaysOff : QQC2.ScrollBar.AsNeeded
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
        section.delegate: Item {
            id: heading
            required property string section
            width: ListView.view.width
            implicitHeight: sidebar.compact ? Kirigami.Units.largeSpacing * 2 : headingText.implicitHeight
            Accessible.role: Accessible.Heading
            Accessible.name: heading.section

            UqLabel {
                id: headingText
                visible: !sidebar.compact
                width: parent.width
                topPadding: Kirigami.Units.largeSpacing
                bottomPadding: Kirigami.Units.smallSpacing
                leftPadding: Kirigami.Units.largeSpacing
                text: heading.section
                font.bold: true
                opacity: 0.7
                elide: Text.ElideRight
                Accessible.ignored: true
            }
            Kirigami.Separator {
                visible: sidebar.compact
                anchors.centerIn: parent
                width: parent.width - Kirigami.Units.largeSpacing * 2
            }
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
            QQC2.ToolTip.visible: sidebar.compact && item.hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            QQC2.ToolTip.text: item.name

            contentItem: RowLayout {
                spacing: Kirigami.Units.smallSpacing
                Kirigami.Icon {
                    source: item.iconName
                    implicitWidth: Kirigami.Units.iconSizes.smallMedium
                    implicitHeight: Kirigami.Units.iconSizes.smallMedium
                    Layout.alignment: sidebar.compact ? Qt.AlignHCenter : Qt.AlignLeft
                    Layout.preferredWidth: implicitWidth
                    Layout.preferredHeight: implicitHeight
                    Accessible.ignored: true
                }
                UqLabel {
                    visible: !sidebar.compact
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
