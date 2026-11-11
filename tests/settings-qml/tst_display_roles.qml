import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 700
    height: 500

    readonly property var screens: [{ name: "eDP-1", label: "BOE 0x0BCA" }, { name: "DP-1", label: "Dell U2723QE" }]

    Component {
        id: comp
        DisplayRoles {
            width: 600
            screens: root.screens
        }
    }

    Component {
        id: spyComp
        SignalSpy {}
    }

    TestCase {
        name: "DisplayRoles"
        when: windowShown

        function make(props) {
            const c = createTemporaryObject(comp, root, props || {});
            verify(c);
            return c;
        }
        function spy(c, name) {
            const s = createTemporaryObject(spyComp, root, { target: c, signalName: name });
            verify(s.valid, name);
            return s;
        }

        function test_choose_shows_checkboxes() {
            const all = make({ bar: { show: "all", outputs: [] } });
            verify(!findChild(all, "barScreens").visible);
            const chosen = make({ bar: { show: "chosen", outputs: ["DP-1"] } });
            verify(findChild(chosen, "barScreens").visible);
            compare(chosen.barRepeater.count, 2);
            verify(!chosen.barRepeater.itemAt(0).checked);
            verify(chosen.barRepeater.itemAt(1).checked);
            // The last display with the bar can't be switched off.
            verify(!chosen.barRepeater.itemAt(1).enabled);
            verify(chosen.barRepeater.itemAt(0).enabled);
        }

        function test_choices_are_sent() {
            const c = make({ bar: { show: "chosen", outputs: ["DP-1"] }, main: null });
            const bar = spy(c, "barChosen");
            const main = spy(c, "mainChosen");
            c.barRepeater.itemAt(0).click();
            compare(bar.signalArguments[0][0], "chosen");
            compare(bar.signalArguments[0][1], ["eDP-1", "DP-1"]);
            c.barBox.activated(1);
            compare(bar.signalArguments[1][0], "main");
            c.mainBox.activated(2);
            compare(main.signalArguments[0][0], "DP-1");
            c.mainBox.activated(0);
            compare(main.signalArguments[1][0], "");
        }

        function test_choose_starts_with_every_display() {
            const c = make({ bar: { show: "all", outputs: [] } });
            const bar = spy(c, "barChosen");
            c.barBox.activated(2);
            compare(bar.signalArguments[0][0], "chosen");
            compare(bar.signalArguments[0][1], ["eDP-1", "DP-1"]);
        }

        function test_main_shown_in_the_box() {
            const c = make({ main: { output: "DP-1", label: "Dell U2723QE", connected: true } });
            compare(c.mainBox.currentIndex, 2);
            verify(!c.missingLabel.visible);
        }

        function test_disconnected_main_says_so() {
            const c = make({ main: { output: null, label: "<b>Dell</b>", connected: false } });
            verify(c.missingLabel.visible);
            verify(c.missingLabel.text.indexOf("<b>Dell</b>") >= 0);
            compare(c.missingLabel.textFormat, Text.PlainText);
            compare(c.mainBox.currentIndex, 0);
        }
    }
}
