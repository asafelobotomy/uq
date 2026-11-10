import QtQuick
import QtTest
import "../../apps/uq-settings/qml"

TestCase {
    id: tc
    name: "Search"
    visible: true
    when: windowShown
    width: 500
    height: 400

    Component {
        id: searchComp
        Search {
            x: 10
            y: 10
            width: 300
            searchFunction: q => q === "zzz" ? "[]" : JSON.stringify([
                { pageId: "p1", settingId: "s1", label: "First", pageName: "Page One" },
                { pageId: "p2", settingId: "s2", label: "Second", pageName: "Page Two" },
                { pageId: "p3", settingId: "", label: "Third", pageName: "Page Three" }
            ])
        }
    }
    Component {
        id: spyComp
        SignalSpy {}
    }

    function make() {
        const s = createTemporaryObject(searchComp, tc);
        verify(s);
        s.forceActiveFocus();
        verify(s.activeFocus);
        return s;
    }

    function test_empty_query_shows_nothing() {
        const s = make();
        compare(s.hits.length, 0);
        verify(!s.open);
    }

    function test_typing_does_not_navigate() {
        const s = make();
        const spy = createTemporaryObject(spyComp, tc, { target: s, signalName: "activated" });
        keyClick("a");
        keyClick("b");
        wait(900); // longer than the field's auto-accept delay
        compare(spy.count, 0);
        compare(s.text, "ab");
        verify(s.open);
        compare(s.hits.length, 3);
    }

    function test_down_enter_chooses_second_and_clears() {
        const s = make();
        const spy = createTemporaryObject(spyComp, tc, { target: s, signalName: "activated" });
        keyClick("a");
        keyClick(Qt.Key_Down);
        keyClick(Qt.Key_Enter);
        compare(spy.count, 1);
        compare(spy.signalArguments[0][0], "p2");
        compare(spy.signalArguments[0][1], "s2");
        compare(s.text, "");
        verify(!s.open);
    }

    function test_escape_closes_and_down_reopens() {
        const s = make();
        keyClick("a");
        verify(s.open);
        keyClick(Qt.Key_Escape);
        verify(!s.open);
        compare(s.text, "a");
        keyClick(Qt.Key_Down);
        verify(s.open);
    }

    function test_empty_state_is_visible() {
        const s = make();
        for (const c of "zzz")
            keyClick(c);
        compare(s.hits.length, 0);
        verify(s.open);
        verify(s.emptyLabel.visible);
        verify(s.resultsPopup.height > 0);
        verify(s.emptyLabel.height > 0 && s.emptyLabel.y >= 0);
    }
}
