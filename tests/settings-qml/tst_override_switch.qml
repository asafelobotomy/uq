import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "OverrideSwitch"
    visible: true
    when: windowShown
    width: 500
    height: 200

    Component {
        id: comp
        OverrideSwitch {
            width: 500
            label: "Blur"
            fallback: true
        }
    }

    function test_ticking_use_default_clears() {
        const s = createTemporaryObject(comp, tc, { value: false });
        const got = [];
        s.chosen.connect(v => got.push(v));
        s.useDefaultBox.toggle();
        s.useDefaultBox.toggled();
        compare(got, [null]);
        compare(s.useDefaultBox.checked, false, "shows the saved value until the model confirms");
    }

    function test_toggling_switch_sends_value() {
        const s = createTemporaryObject(comp, tc, { value: false });
        const got = [];
        s.chosen.connect(v => got.push(v));
        s.toggle.toggle();
        s.toggle.toggled();
        compare(got, [true]);
        compare(s.toggle.checked, false, "snaps back to the saved value");
    }

    function test_untick_seeds_from_fallback_and_default_shows_fallback() {
        const s = createTemporaryObject(comp, tc, { value: null });
        compare(s.toggle.checked, true, "no override: shows the default");
        compare(s.toggle.enabled, false);
        const got = [];
        s.chosen.connect(v => got.push(v));
        s.useDefaultBox.toggle();
        s.useDefaultBox.toggled();
        compare(got, [true]);
    }
}
