import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "WallpaperEntryEditor"
    visible: true
    when: windowShown
    width: 700
    height: 500

    Component {
        id: comp
        WallpaperEntryEditor {
            width: 700
        }
    }

    function test_refresh_with_same_content_keeps_edits() {
        const ed = createTemporaryObject(comp, tc, { entry: { image: "/a.png", fit: "fill", interval: 300 } });
        compare(ed.imageField.text, "/a.png");
        ed.imageField.text = "/half/typed";
        ed.entry = { image: "/a.png", fit: "fill", interval: 300 };
        compare(ed.imageField.text, "/half/typed");
        ed.entry = { image: "/b.png", fit: "fill", interval: 300 };
        compare(ed.imageField.text, "/b.png");
    }

    function test_interval_is_clamped_and_emitted() {
        const ed = createTemporaryObject(comp, tc, { entry: { folder: "/pics", interval: 300 } });
        const got = [];
        ed.changed.connect(e => got.push(e));
        compare(ed.minutesBox.value, 5);
        compare(ed.secondsBox.value, 0);
        ed.minutesBox.value = 0;
        ed.secondsBox.value = 3;
        ed.applyInterval();
        compare(got.length, 1);
        compare(got[0].interval, 10);
        compare(got[0].folder, undefined);
        compare(ed.secondsBox.value, 10);
        ed.minutesBox.value = 1440;
        ed.secondsBox.value = 59;
        ed.applyInterval();
        compare(got[1].interval, 86400);
    }

    function test_color_needs_a_valid_hex() {
        const ed = createTemporaryObject(comp, tc, { entry: {} });
        const got = [];
        ed.changed.connect(e => got.push(e));
        ed.colorField.text = "#12";
        ed.applyColor();
        compare(got.length, 0);
        ed.colorField.text = "#A0B0C0";
        ed.applyColor();
        compare(got.length, 1);
        compare(got[0].color, "#a0b0c0");
        ed.colorField.text = "";
        ed.applyColor();
        compare(got[1].color, null);
    }

    function test_span_choice_only_for_all_displays() {
        const all = createTemporaryObject(comp, tc, { output: "*", entry: { image: "/a.png", fit: "span" } });
        compare(all.fitField.model.length, 5);
        compare(all.fitField.model[4], "Span across displays");
        tryCompare(all.fitField, "currentIndex", 4);
        const one = createTemporaryObject(comp, tc, { output: "DP-1", entry: { image: "/a.png", fit: "tile" } });
        compare(one.fitField.model.length, 4);
        compare(one.fitField.model.indexOf("Span across displays"), -1);
        // A display copying a spanning All displays entry shows a plain fit, never span.
        one.entry = { image: "/a.png", fit: "span" };
        tryCompare(one.fitField, "currentIndex", 0);
        // Switching the editor between displays changes the choices.
        one.output = "*";
        compare(one.fitField.model.length, 5);
    }

    function test_fit_buttons_send_the_fit() {
        const ed = createTemporaryObject(comp, tc, { output: "*", entry: { fit: "fit" } });
        const got = [];
        ed.changed.connect(e => got.push(e));
        tryCompare(ed.fitField, "currentIndex", 1);
        const tile = findChild(ed, "fit-tile");
        verify(tile);
        verify(findChild(ed, "fit-fit").checked);
        tile.clicked();
        compare(got.length, 1);
        compare(got[0].fit, "tile");
        // Until uqd saves it, the saved fit stays chosen.
        compare(ed.fitField.currentIndex, 1);
    }
}
