import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 600
    height: 400

    Component {
        id: comp
        RevertDialog {}
    }

    TestCase {
        id: tc
        name: "RevertDialog"
        when: windowShown

        function make(seconds) {
            const d = createTemporaryObject(comp, root, { seconds: seconds });
            d.active = true;
            tryCompare(d, "opened", true);
            return d;
        }

        function test_counts_down() {
            const d = make(3);
            compare(d.message, "Reverting in 3 seconds.");
            wait(1100);
            verify(d.message.indexOf("2") >= 0, d.message);
            compare(d.title, "Keep these display settings?");
        }

        function test_enter_does_not_keep() {
            const d = make(10);
            let kept = 0;
            d.keep.connect(() => kept++);
            keyClick(Qt.Key_Return);
            keyClick(Qt.Key_Enter);
            compare(kept, 0);
            verify(d.opened);
        }

        function test_escape_reverts() {
            const d = make(10);
            let reverted = 0;
            d.revert.connect(() => reverted++);
            keyClick(Qt.Key_Escape);
            compare(reverted, 1);
        }

        function test_buttons() {
            const d = make(10);
            let kept = 0, reverted = 0;
            d.keep.connect(() => kept++);
            d.revert.connect(() => reverted++);
            d.keepButton.clicked();
            d.revertButton.clicked();
            compare([kept, reverted], [1, 1]);
        }

        function test_zero_does_not_call_revert() {
            const d = make(1);
            let reverted = 0;
            d.revert.connect(() => reverted++);
            tryCompare(d, "left", 0, 3000);
            compare(d.message, "Reverting…");
            wait(1200);
            compare(d.left, 0, "never below 0");
            compare(reverted, 0, "the service reverts by itself");
        }

        function test_closing_when_kept_does_not_revert() {
            const d = make(10);
            let reverted = 0;
            d.revert.connect(() => reverted++);
            d.active = false;
            tryCompare(d, "opened", false);
            compare(reverted, 0);
        }
    }
}
