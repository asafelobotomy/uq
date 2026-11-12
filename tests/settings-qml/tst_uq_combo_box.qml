import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "UqComboBox"
    visible: true
    when: windowShown
    width: 300
    height: 100

    Component {
        id: comp
        UqComboBox {
            model: ["Fill the screen", "Fit"]
        }
    }

    function test_accessible_description_follows_selection() {
        const c = createTemporaryObject(comp, tc);
        verify(c);
        compare(c.Accessible.role, Accessible.ComboBox);
        compare(c.Accessible.description, "Fill the screen");
        c.currentIndex = 1;
        compare(c.Accessible.description, "Fit");
        compare(c.Accessible.description, c.currentText);
    }

    // qqc2-desktop-style (the app's style; run with QT_QUICK_CONTROLS_STYLE=org.kde.desktop to
    // exercise it) asks the contentItem for positionToRectangle; the label answers with an
    // empty rectangle instead of throwing a TypeError.
    function test_label_answers_the_style_cursor_calls() {
        failOnWarning(/positionToRectangle/);
        const c = createTemporaryObject(comp, tc);
        verify(c);
        compare(c.contentItem.selectionStart, 0);
        const r = c.contentItem.positionToRectangle(0);
        compare(r.width, 0);
        compare(r.height, 0);
    }

    Component {
        id: longComp
        UqComboBox {
            width: 120
            model: ["On", "A much longer choice than the box is wide"]
        }
    }

    // The popup is never narrower than the box, and its rows fill it.
    function test_popup_is_at_least_as_wide_as_the_box() {
        const c = createTemporaryObject(comp, tc, { width: 200 });
        c.popup.open();
        tryVerify(() => c.popup.opened);
        verify(c.popup.width >= c.width, "popup " + c.popup.width + " < box " + c.width);
        const row = c.popup.contentItem.itemAtIndex
            ? c.popup.contentItem.itemAtIndex(0) : c.popup.itemAt(0);
        verify(row);
        verify(Math.abs(row.width - c.popup.availableWidth) <= 2,
            "row " + row.width + " vs popup " + c.popup.availableWidth);
        c.popup.close();
        const l = createTemporaryObject(longComp, tc);
        l.popup.open();
        tryVerify(() => l.popup.opened);
        // The KDE style sizes its popup to the widest row; Basic keeps the box's width.
        if (l.popup.implicitWidth > l.width)
            verify(l.popup.width > l.width, "a long choice widens the popup");
        l.popup.close();
    }

    // A highlighted row's text switches to the highlighted text colour.
    function test_highlighted_row_uses_highlighted_text() {
        const c = createTemporaryObject(comp, tc);
        c.popup.open();
        tryVerify(() => c.popup.opened);
        const row = c.popup.contentItem.itemAtIndex
            ? c.popup.contentItem.itemAtIndex(0) : c.popup.itemAt(0);
        row.highlighted = false;
        verify(!row.contentItem.highlighted);
        const plain = String(row.contentItem.color); // a copy, not a live reference
        row.highlighted = true;
        verify(row.contentItem.highlighted);
        verify(String(row.contentItem.color) !== plain, "the text colour changes");
        c.popup.close();
    }
}
