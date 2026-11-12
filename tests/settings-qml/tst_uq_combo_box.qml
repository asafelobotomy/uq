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
}
