import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

// Welcome's navigation (Phase 5 spec §5.1): Back is off on the first step, every step but the
// last can be skipped, and the last one shows Finish instead of Next and Skip.
TestCase {
    id: tc
    name: "WelcomeNav"
    visible: true
    when: windowShown
    width: 600
    height: 100

    Component {
        id: comp
        WelcomeNav {}
    }

    function child(item, name) {
        return findChild(item, name);
    }

    SignalSpy { id: backSpy; signalName: "back" }
    SignalSpy { id: skipSpy; signalName: "skip" }
    SignalSpy { id: nextSpy; signalName: "next" }
    SignalSpy { id: finishSpy; signalName: "finish" }

    function make(index, count) {
        const nav = createTemporaryObject(comp, tc, { index: index, count: count, width: 600 });
        for (const s of [backSpy, skipSpy, nextSpy, finishSpy]) {
            s.clear();
            s.target = nav;
        }
        return nav;
    }

    function test_first_step() {
        const nav = make(0, 6);
        verify(!child(nav, "backButton").enabled);
        verify(child(nav, "skipButton").visible);
        verify(child(nav, "nextButton").visible);
        verify(!child(nav, "finishButton").visible);
        compare(child(nav, "stepCount").text, "Step 1 of 6");
        mouseClick(child(nav, "nextButton"));
        compare(nextSpy.count, 1);
        mouseClick(child(nav, "skipButton"));
        compare(skipSpy.count, 1);
    }

    function test_middle_step_goes_back() {
        const nav = make(2, 6);
        verify(child(nav, "backButton").enabled);
        mouseClick(child(nav, "backButton"));
        compare(backSpy.count, 1);
        compare(child(nav, "stepCount").text, "Step 3 of 6");
    }

    function test_last_step_finishes() {
        const nav = make(5, 6);
        verify(!child(nav, "skipButton").visible);
        verify(!child(nav, "nextButton").visible);
        verify(child(nav, "finishButton").visible);
        mouseClick(child(nav, "finishButton"));
        compare(finishSpy.count, 1);
        compare(nextSpy.count, 0);
    }

    function test_busy_holds_the_buttons() {
        const nav = make(1, 6);
        nav.busy = true;
        verify(!child(nav, "nextButton").enabled);
        verify(!child(nav, "skipButton").enabled);
        verify(!child(nav, "backButton").enabled);
        // Finish never waits: closing is always allowed.
        nav.index = 5;
        verify(child(nav, "finishButton").enabled);
    }
}
