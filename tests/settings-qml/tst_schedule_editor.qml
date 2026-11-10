import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "ScheduleEditor"
    visible: true
    when: windowShown
    width: 600
    height: 300

    Component {
        id: comp
        ScheduleEditor {
            width: 600
        }
    }

    function test_refresh_with_same_content_keeps_edits() {
        const ed = createTemporaryObject(comp, tc, { schedule: { mode: "off" } });
        compare(ed.mode, "off");
        ed.mode = "fixed";
        ed.lightField.text = "06:3";
        // uqd's state is re-read: same content, a brand-new object.
        ed.schedule = { mode: "off" };
        compare(ed.mode, "fixed");
        compare(ed.lightField.text, "06:3");
    }

    function test_changed_saved_schedule_reloads() {
        const ed = createTemporaryObject(comp, tc, { schedule: { mode: "off" } });
        ed.mode = "sun";
        ed.latField.text = "9";
        ed.schedule = { mode: "fixed", light_at: "08:15:00", dark_at: "20:00:00" };
        compare(ed.mode, "fixed");
        compare(ed.lightField.text, "08:15");
        compare(ed.darkField.text, "20:00");
        compare(ed.latField.text, "");
    }

    function test_apply_emits_json() {
        const ed = createTemporaryObject(comp, tc, { schedule: { mode: "sun", lat: 51.5, lon: -0.13 } });
        const got = [];
        ed.apply.connect(j => got.push(j));
        compare(ed.latField.text, "51.5");
        ed.mode = "fixed";
        ed.lightField.text = "07:30";
        ed.darkField.text = "19:00";
        const buttons = [];
        (function walk(i) { for (const c of i.children) { if (c.Accessible && c.Accessible.name === "Apply times") buttons.push(c); walk(c); } })(ed);
        compare(buttons.length, 1);
        buttons[0].clicked();
        compare(got, ['{"mode":"fixed","light_at":"07:30:00","dark_at":"19:00:00"}']);
    }
}
