import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import QtTest
import org.kde.kirigami as Kirigami
import "../../apps/uq-settings/qml/components"

// The shared width rule: cards never stretch past SettingsTab.columnWidth, a second column
// appears only once there is room for two, and a wide card puts its plain rows two to a line.
TestCase {
    id: tc
    name: "SettingsTabWidth"
    visible: true
    when: windowShown
    width: 2000
    height: 900

    Component {
        id: tabComp
        SettingsTab {
            id: tab
            property alias first: first
            property alias second: second
            property alias wideCard: wideCard
            property alias rowA: rowA
            property alias rowB: rowB
            property alias list: list
            SettingsCard {
                id: first
                title: "One"
                SettingRow { label: "A"; Layout.fillWidth: true; QQC2.Switch {} }
            }
            SettingsCard {
                id: second
                title: "Two"
                SettingRow { label: "B"; Layout.fillWidth: true; QQC2.Switch {} }
            }
            SettingsCard {
                id: wideCard
                title: "Wide"
                wide: true
                SettingRow { id: rowA; label: "A"; QQC2.Switch {} }
                SettingRow { id: rowB; label: "B"; QQC2.Switch {} }
                Item { id: list; Layout.fillWidth: true; implicitHeight: 20 }
            }
        }
    }

    function make(w) {
        const t = createTemporaryObject(tabComp, tc, { width: w, height: 600 });
        verify(t);
        wait(50);
        return t;
    }

    function test_narrow_window_is_one_column() {
        const t = make(1000);
        compare(t.columns, 1);
        verify(t.first.width <= t.width);
        compare(t.first.x, t.second.x);
    }

    function test_cards_never_exceed_the_column_width() {
        for (const w of [1100, 1500, 1900, 2000]) {
            const t = make(w);
            verify(t.first.width <= t.columnWidth + 1, "card " + t.first.width + " at window " + w);
            verify(t.second.width <= t.columnWidth + 1);
        }
    }

    function test_wide_window_is_two_columns() {
        const t = make(1900);
        compare(t.columns, 2);
        verify(t.second.x > t.first.x);
        compare(t.first.y, t.second.y);
    }

    function test_one_column_block_is_centred() {
        const t = make(1100);
        compare(t.columns, 1);
        const left = t.first.mapToItem(t, 0, 0).x;
        const right = t.width - (left + t.first.width);
        verify(Math.abs(left - right) <= Math.max(Kirigami.Units.gridUnit, 20), "left " + left + " right " + right);
    }

    function test_wide_card_pairs_plain_rows() {
        const t = make(1900);
        verify(t.wideCard.width > t.columnWidth);
        compare(t.rowA.mapToItem(t.wideCard, 0, 0).y, t.rowB.mapToItem(t.wideCard, 0, 0).y);
        verify(t.rowB.x > t.rowA.x);
        verify(t.rowA.width <= t.columnWidth + 1);
        compare(t.list.width, t.wideCard.width - Kirigami.Units.largeSpacing * 4);
    }

    function test_wide_card_in_one_column_stacks_rows() {
        const t = make(1000);
        verify(t.rowB.mapToItem(t.wideCard, 0, 0).y > t.rowA.mapToItem(t.wideCard, 0, 0).y);
    }

    function test_row_caps_itself() {
        const col = createTemporaryObject(rowComp, tc);
        const row = col.row;
        verify(row);
        wait(30);
        verify(row.width <= Kirigami.Units.gridUnit * 44 + 1);
    }

    Component {
        id: rowComp
        ColumnLayout {
            property alias row: inner
            width: 1800
            SettingRow { id: inner; label: "Lonely"; Layout.fillWidth: true; QQC2.Switch {} }
        }
    }
}
