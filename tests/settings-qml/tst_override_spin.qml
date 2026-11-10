import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "OverrideSpin"
    visible: true
    when: windowShown
    width: 500
    height: 200

    Component {
        id: comp
        OverrideSpin {
            width: 500
            label: "Size"
            from: 6
            to: 32
            fallback: 13
        }
    }

    function test_held_arrow_emits_once() {
        const s = createTemporaryObject(comp, tc, { value: 10 });
        const got = [];
        s.chosen.connect(v => got.push(v));
        s.spinBox.forceActiveFocus();
        keyClick(Qt.Key_Up);
        keyClick(Qt.Key_Up);
        keyClick(Qt.Key_Up);
        compare(got.length, 0, "nothing is sent while the value is still moving");
        tryCompare(got, "length", 1, 2000);
        compare(got[0], 13);
        wait(500);
        compare(got.length, 1);
    }

    function test_untick_seeds_from_fallback_and_cancels_pending() {
        const s = createTemporaryObject(comp, tc, { value: null });
        const got = [];
        s.chosen.connect(v => got.push(v));
        s.useThemeBox.toggle();
        s.useThemeBox.toggled();
        compare(got, [13]);

        const t = createTemporaryObject(comp, tc, { value: 10 });
        const got2 = [];
        t.chosen.connect(v => got2.push(v));
        t.spinBox.forceActiveFocus();
        keyClick(Qt.Key_Up);
        t.useThemeBox.toggle();
        t.useThemeBox.toggled();
        wait(600);
        compare(got2, [null], "ticking Use theme drops the pending number");
    }
}
