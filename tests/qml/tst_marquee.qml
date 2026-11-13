import QtQuick
import QtTest
import "../../shell/core"
import "../../shell/bar/islands"

// The marquee scrolls only when its items don't fit, and never with reduced motion, while
// hovered or while the screen is shared; then it steps through the items instead.
TestCase {
    name: "marquee"
    width: 400
    height: 100
    when: windowShown

    Component {
        id: maker
        Marquee {
            width: 80
            height: 20
            delegate: Rectangle {
                required property var modelData
                width: modelData
                height: 10
            }
        }
    }

    function test_fits_does_not_move() {
        var m = createTemporaryObject(maker, this, { model: [20, 20], reduceMotion: false, sharing: false });
        verify(!m.overflows);
        verify(!m.moving);
        verify(!m.stepping);
    }

    function test_overflow_moves_and_pauses() {
        var m = createTemporaryObject(maker, this, { model: [60, 60], reduceMotion: false, sharing: false });
        verify(m.overflows);
        verify(m.moving);
        m.focused = true;
        verify(!m.moving, "keyboard focus pauses");
        m.focused = false;
        m.sharing = true;
        verify(!m.moving);
        verify(m.stepping, "while sharing it steps instead");
    }

    function test_reduced_motion_steps() {
        var m = createTemporaryObject(maker, this, { model: [60, 60, 60], reduceMotion: true, sharing: false, stepMs: 50 });
        verify(m.overflows);
        verify(!m.moving);
        verify(m.stepping);
        compare(m.stepIndex, 0);
        tryVerify(function () { return m.stepIndex > 0; }, 1000);
        m.model = [60, 60];
        compare(m.stepIndex, 0, "a new model starts over");
    }
}
