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

    Component {
        id: spyComp
        SignalSpy {}
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

        function floatRule(e) {
            e.nameField.text = "A floats";
            tick(e, "effectOn_float");
            type(e, "matchValue_class", "^a$");
        }

        function test_invalid_preview_disables_save() {
            const e = make("window");
            floatRule(e);
            verify(e.saveEnabled);
            e.previewResult = { valid: false, error: "this pattern can't be used", matches: [] };
            verify(!e.saveEnabled, "Save waits for a draft that can be saved");
            const spy = createTemporaryObject(spyComp, root, { target: e, signalName: "ruleAccepted" });
            e.save();
            compare(spy.count, 0);
        }

        function test_save_waits_for_the_answer_and_keeps_a_refused_draft() {
            const e = make("window");
            floatRule(e);
            const spy = createTemporaryObject(spyComp, root, { target: e, signalName: "ruleAccepted" });
            e.save();
            compare(spy.count, 1);
            compare(spy.signalArguments[0][0], "window");
            compare(spy.signalArguments[0][2], "", "a new rule replaces nothing");
            verify(e.saving);
            verify(!e.saveEnabled, "no second Save while the first is out");
            e.finishSave("<b>Hyprland</b> said no");
            verify(!e.saving);
            compare(e.saveError, "That wasn't saved: <b>Hyprland</b> said no");
            compare(control(e, "saveError").textFormat, Text.PlainText);
            compare(e.nameField.text, "A floats", "the draft is still there");
            verify(control(e, "effectOn_float").checked);
            e.save();
            compare(spy.count, 2);
            e.finishSave("");
            verify(!e.saving);
        }

        function test_check_problems_stop_the_save() {
            const e = make("workspace");
            e.check = (kind, draft, replaces) => JSON.stringify({ valid: false, problems: ["Workspace 3 already has a rule; edit that one."] });
            e.selectorField.text = "3";
            tick(e, "effectOn_persistent");
            const spy = createTemporaryObject(spyComp, root, { target: e, signalName: "ruleAccepted" });
            e.save();
            compare(spy.count, 0);
            verify(!e.saving);
            compare(e.saveError, "This rule can't be saved yet: Workspace 3 already has a rule; edit that one.");
        }

        function test_edit_says_what_it_replaces() {
            const e = make("workspace", { workspace: "name:web", persistent: true });
            compare(e.replaces, "name:web");
            e.load("window", { id: "r-00ab", name: "A", enabled: true, match: { "class": "^a$" }, effects: { "float": true } });
            compare(e.replaces, "r-00ab");
        }

        function test_false_effect_starts_unticked() {
            const e = make("window", { id: "r-00ab", name: "A", enabled: true, match: { "class": "^a$" }, effects: { "float": false, "center": true } });
            verify(!control(e, "effectOn_float").checked, "an effect stored as false does nothing");
            compare(JSON.parse(e.ruleJson()).effects, { "center": true });
        }

        /// The page's request: busy goes true, then false, with no change of state.
        function request(e) {
            e.busy = true;
            e.canSave = false;
            e.busy = false;
            e.canSave = true;
        }

        function test_save_ends_on_busy_even_with_an_unchanged_state() {
            const e = make("window");
            floatRule(e);
            e.errorNow = () => "<i>no</i> such thing";
            for (let i = 0; i < 2; i++) {
                e.save();
                verify(e.saving, "round " + i);
                verify(!control(e, "editorPick").enabled);
                request(e);
                tryCompare(e, "saving", false);
                compare(e.saveError, "That wasn't saved: <i>no</i> such thing", "the same refusal shows again");
                verify(control(e, "editorPick").enabled, "Pick works again");
                verify(e.saveEnabled);
                e.finishSave("late");
                compare(e.saveError, "That wasn't saved: <i>no</i> such thing", "a second finish does nothing");
            }
            e.errorNow = () => "";
            e.open();
            tryVerify(() => e.opened);
            e.save();
            request(e);
            tryCompare(e, "saving", false);
            tryVerify(() => !e.opened, 3000, "a saved rule closes the editor");
            compare(e.saveError, "");
        }

        function test_pick_follows_busy() {
            const e = make("window");
            verify(control(e, "editorPick").enabled);
            e.canSave = false;
            verify(!control(e, "editorPick").enabled);
        }
    }
}
