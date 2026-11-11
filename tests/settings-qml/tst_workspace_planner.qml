import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 700
    height: 800

    readonly property var screens: [{ output: "eDP-1", label: "BOE 0x0BCA" }, { output: "DP-1", label: "Dell U2723QE" }]

    Component {
        id: comp
        WorkspacePlanner {
            width: 600
        }
    }

    Component {
        id: spyComp
        SignalSpy {}
    }

    TestCase {
        name: "WorkspacePlanner"
        when: windowShown

        function make(plan) {
            const c = createTemporaryObject(comp, root, { plan: Object.assign({ strategy: "off", count: 10, move_back: true, manual: [], screens: root.screens, preview: [] }, plan) });
            verify(c);
            return c;
        }
        function spy(c, name) {
            const s = createTemporaryObject(spyComp, root, { target: c, signalName: name });
            verify(s.valid, name);
            return s;
        }

        function test_preview_rows() {
            const c = make({
                strategy: "sequential",
                preview: [{ output: "eDP-1", label: "BOE 0x0BCA", workspaces: [1, 2, 3, 4, 5] }, { output: "DP-1", label: "<b>Dell</b>", workspaces: [6, 7, 8, 9, 10] }]
            });
            compare(c.previewRepeater.count, 2);
            compare(c.previewRepeater.itemAt(0).screenLabel, "BOE 0x0BCA");
            compare(c.previewRepeater.itemAt(0).workspacesText, "Workspaces 1 to 5");
            compare(c.previewRepeater.itemAt(1).workspacesText, "Workspaces 6 to 10");
            // A screen's name stays plain text.
            const label = findChild(c.previewRepeater.itemAt(1), "previewScreen");
            compare(label.text, "<b>Dell</b>");
            compare(label.textFormat, Text.PlainText);
        }

        function test_preview_hidden_when_off() {
            const c = make({ strategy: "off", preview: [] });
            compare(c.previewRepeater.count, 0);
            verify(!findChild(c, "preview").visible);
            verify(!findChild(c, "countRow").visible);
            verify(!c.backSwitch.visible);
        }

        function test_describe_lists() {
            const c = make();
            compare(c.describe([3]), "3");
            compare(c.describe([1, 3, 5]), "1, 3, 5");
            compare(c.describe([2, 3, 4]), "2 to 4");
        }

        function test_manual_combos_only_for_choose() {
            const sequential = make({ strategy: "sequential" });
            compare(sequential.manualRepeater.count, 0);
            const manual = make({ strategy: "manual", count: 4, manual: [{ workspace: 2, output: "DP-1", label: "Dell U2723QE" }] });
            compare(manual.manualRepeater.count, 4);
            const second = findChild(manual.manualRepeater.itemAt(1), "manualBox");
            compare(second.currentIndex, 2, "workspace 2 is on the second screen");
            const first = findChild(manual.manualRepeater.itemAt(0), "manualBox");
            compare(first.currentIndex, 0);
        }

        function test_choices_are_sent() {
            const c = make({ strategy: "manual", count: 3 });
            const plan = spy(c, "planChosen");
            const manual = spy(c, "manualChosen");
            c.strategyBox.activated(1);
            compare(plan.count, 1);
            compare(plan.signalArguments[0], ["sequential", 3, true]);
            c.countBox.value = 6;
            c.countBox.valueModified();
            compare(plan.signalArguments[1], ["manual", 6, true]);
            c.backSwitch.chosen(false);
            compare(plan.signalArguments[2], ["manual", 3, false]);
            findChild(c.manualRepeater.itemAt(2), "manualBox").activated(1);
            compare(manual.signalArguments[0], [3, "eDP-1"]);
            findChild(c.manualRepeater.itemAt(2), "manualBox").activated(0);
            compare(manual.signalArguments[1], [3, ""]);
        }

        function test_manual_choice_survives_a_new_screen_list() {
            const c = make({ strategy: "manual", count: 2, manual: [{ workspace: 1, output: "eDP-1", label: "BOE 0x0BCA" }] });
            const box = findChild(c.manualRepeater.itemAt(0), "manualBox");
            compare(box.currentIndex, 1);
            c.plan = Object.assign({}, c.plan, { screens: root.screens.concat([{ output: "DP-2", label: "Other" }]) });
            tryCompare(findChild(c.manualRepeater.itemAt(0), "manualBox"), "currentIndex", 1);
        }

        function test_count_box_snaps_back_after_a_refusal() {
            const c = make({ strategy: "sequential", count: 10 });
            c.countBox.value = 9;
            c.countBox.valueModified();
            compare(c.countBox.value, 10);
        }
    }
}
