import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"
import "fixtures/rule_fields.js" as Fixture

Item {
    id: root
    width: 900
    height: 700

    Component {
        id: comp
        RuleEditor {
            fields: Fixture.fields
            exact: text => "^stub:" + text + "$"
        }
    }

    TestCase {
        name: "RuleEditor"
        when: windowShown

        function make(kind, raw) {
            const e = createTemporaryObject(comp, root);
            verify(e);
            e.load(kind || "window", raw === undefined ? null : raw);
            return e;
        }
        function control(e, name) {
            const c = findChild(e.body, name);
            verify(c, "no control " + name);
            return c;
        }
        function tick(e, name) {
            const box = control(e, name);
            box.toggle();
            box.toggled();
        }
        function type(e, name, text) {
            const field = control(e, name);
            field.text = text;
            field.textEdited();
        }

        function test_checked_fields_build_json() {
            const e = make("window");
            e.nameField.text = "Firefox floats";
            tick(e, "effectOn_float");
            tick(e, "effectOn_size");
            const width = control(e, "effectA_size");
            width.value = 1024;
            width.valueModified();
            type(e, "matchValue_class", "^firefox$");
            compare(JSON.parse(e.ruleJson()), {
                name: "Firefox floats",
                enabled: true,
                match: { "class": "^firefox$" },
                effects: { "float": true, "size": [1024, 600] }
            });
            verify(control(e, "matchOn_class").checked, "typing a pattern includes the field");
        }

        function test_exact_text_helper_calls_exact() {
            const e = make("window");
            type(e, "matchValue_class", "Firefox");
            control(e, "matchExact_class").clicked();
            compare(control(e, "matchValue_class").text, "^stub:Firefox$");
            compare(JSON.parse(e.ruleJson()).match["class"], "^stub:Firefox$");
        }

        function test_float_and_tile_exclusive() {
            const e = make("window");
            tick(e, "effectOn_float");
            verify(control(e, "effectOn_float").checked);
            tick(e, "effectOn_tile");
            verify(control(e, "effectOn_tile").checked);
            verify(!control(e, "effectOn_float").checked, "turning on Tile turns off Float");
            const effects = JSON.parse(e.ruleJson()).effects;
            compare(effects["tile"], true);
            compare(effects["float"], undefined);
            tick(e, "effectOn_float");
            verify(!control(e, "effectOn_tile").checked, "and the other way round");
        }

        function test_name_limit() {
            const e = make("window");
            e.nameField.text = "x".repeat(150);
            compare(e.nameField.text.length, 100);
            compare(JSON.parse(e.ruleJson()).name.length, 100);
        }

        function test_title_is_fixed() {
            const e = make("window");
            compare(e.title, "New rule");
            e.load("window", {
                id: "r-0001",
                name: "<b>x</b>",
                enabled: true,
                match: { "class": "^a$" },
                effects: { "float": true }
            });
            compare(e.title, "Edit rule");
            compare(e.nameField.text, "<b>x</b>");
        }

        function test_edit_keeps_id_and_switch() {
            const raw = {
                id: "r-00ab",
                name: "Quiet chat",
                enabled: false,
                match: { "class": "^chat$" },
                effects: { "workspace": "3", "silent": true, "opacity": 0.9 }
            };
            const e = make("window", raw);
            verify(control(e, "quietly").checked);
            compare(control(e, "effectNumber_opacity").value, 90);
            compare(JSON.parse(e.ruleJson()), raw);
        }

        function test_workspace_rule_is_flat() {
            const e = make("workspace", { workspace: "3", persistent: true, gaps_in: 5 });
            compare(e.title, "Edit rule");
            verify(e.selectorField.readOnly, "a workspace rule keeps its workspace");
            compare(JSON.parse(e.ruleJson()), { workspace: "3", persistent: true, gaps_in: 5 });
        }
    }
}
