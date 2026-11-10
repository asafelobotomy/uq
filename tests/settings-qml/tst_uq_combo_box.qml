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
}
