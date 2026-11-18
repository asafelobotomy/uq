import QtQuick
import QtTest
import "../../shell/core"

// The shell's motion tokens are zero under ui.reduce_motion and fixed otherwise.
TestCase {
    name: "motion"

    function cleanup() {
        Settings.values = ({});
    }

    function test_default_speeds() {
        compare(Motion.fast, 100);
        compare(Motion.base, 140);
        compare(Motion.easing, Easing.OutCubic);
    }

    function test_reduce_motion_zeroes_durations() {
        Settings.values = ({ ui: { reduce_motion: true } });
        compare(Motion.fast, 0);
        compare(Motion.base, 0);
    }
}
