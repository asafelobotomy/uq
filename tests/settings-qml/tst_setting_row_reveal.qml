import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import QtTest
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "SettingRowReveal"
    visible: true
    when: windowShown
    width: 400
    height: 300

    Component {
        id: page
        QQC2.ScrollView {
            property alias target: far
            property alias near: near
            width: 400
            height: 300
            contentWidth: availableWidth
            ColumnLayout {
                width: parent.width
                SettingRow {
                    id: near
                    settingId: "near"
                    label: "Near"
                    Layout.fillWidth: true
                }
                Item {
                    Layout.preferredHeight: 2000
                }
                SettingRow {
                    id: far
                    settingId: "far"
                    label: "Far"
                    Layout.fillWidth: true
                }
                Item {
                    Layout.preferredHeight: 600
                }
            }
        }
    }

    function inViewport(view, row) {
        const p = row.mapToItem(view, 0, 0);
        return p.y >= 0 && p.y + row.height <= view.height;
    }

    function test_highlight_below_the_fold_scrolls_into_view() {
        const view = createTemporaryObject(page, tc);
        waitForRendering(view);
        verify(!inViewport(view, view.target), "starts below the fold");
        view.target.highlight = "far";
        verify(view.target.flashing);
        tryVerify(() => inViewport(view, view.target), 1000, "highlighted row is scrolled into view");
    }

    function test_visible_row_does_not_scroll() {
        const view = createTemporaryObject(page, tc);
        waitForRendering(view);
        const flick = view.contentItem;
        const before = flick.contentY;
        view.near.highlight = "near";
        wait(50);
        compare(flick.contentY, before);
    }
}
