import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"
import "fixtures/themes.js" as Fixture

Item {
    id: root
    width: 900
    height: 800

    // A stand-in for ThemesModel.preview: the fixture's answer with the typed colours as tokens,
    // and (when `broken` is set) a problem that blocks saving.
    property bool broken: false
    function stubPreview(json) {
        const draft = JSON.parse(json);
        const out = JSON.parse(JSON.stringify(Fixture.preview));
        for (const v of ["dark", "light"]) {
            for (const k of ["bg", "surface", "text", "accent"])
                out[v].tokens[k] = draft[v][k];
        }
        if (root.broken) {
            out.ok = false;
            out.dark.problems = ["Text on the background is hard to read in the dark version (1.0 to 1; it needs 4.5 to 1)."];
        }
        return JSON.stringify(out);
    }

    Component {
        id: comp
        ThemeEditor {
            preview: json => root.stubPreview(json)
        }
    }

    Component {
        id: spyComp
        SignalSpy {}
    }

    TestCase {
        name: "ThemeEditor"
        when: windowShown

        function init() {
            root.broken = false;
        }
        function make(session) {
            const e = createTemporaryObject(comp, root);
            verify(e);
            e.start(session || Fixture.editor);
            return e;
        }
        function control(e, name) {
            const c = findChild(e.body, name);
            verify(c, "no control " + name);
            return c;
        }
        function type(e, name, text) {
            const field = control(e, name);
            field.text = text;
            field.textEdited();
        }

        function test_title_is_fixed() {
            const e = make(Object.assign({}, Fixture.editor, { edit: true, name: "<b>x</b>" }));
            compare(e.title, "Edit theme");
            compare(e.nameField.text, "<b>x</b>");
            const fresh = make();
            compare(fresh.title, "New theme");
            compare(fresh.nameField.text, "Gruvbox copy");
        }

        function test_rows_come_from_the_session_keys() {
            const e = make();
            for (const k of Fixture.editor.keys) {
                verify(control(e, "dark-" + k.key), "no dark-" + k.key);
                verify(control(e, "light-" + k.key), "no light-" + k.key);
            }
            compare(control(e, "dark-bg").text, "#282828");
            compare(control(e, "light-bg").text, "#fbf1c7");
        }

        function test_hex_field_updates_preview() {
            const e = make();
            type(e, "dark-bg", "#112233");
            compare(e.previewResult.dark.tokens.bg, "#112233");
            compare(String(control(e, "previewCard").color), "#112233");
            compare(String(control(e, "previewPanel").color), String(Fixture.preview.dark.tokens.surface));
            // The light version has its own colours; the card follows the open tab.
            compare(e.previewResult.light.tokens.bg, Fixture.editor.colors.light.bg);
        }

        function test_save_disabled_on_problems() {
            const e = make();
            verify(e.saveEnabled);
            root.broken = true;
            type(e, "dark-text", "#282828");
            verify(!e.saveEnabled);
            const problems = control(e, "problems");
            verify(problems.visible);
            verify(problems.text.indexOf("Text on the background") >= 0);
            compare(problems.textFormat, Text.PlainText);
            root.broken = false;
            type(e, "dark-text", "#ebdbb2");
            verify(e.saveEnabled);
            verify(!control(e, "problems").visible);
        }

        function test_save_disabled_when_the_name_has_a_problem() {
            const e = make();
            const bad = JSON.parse(JSON.stringify(Fixture.preview));
            bad.ok = false;
            bad.nameProblem = "A theme needs a name of 1 to 64 characters, without special characters.";
            e.preview = () => JSON.stringify(bad);
            type(e, "themeNameField", "");
            verify(!e.saveEnabled);
            verify(control(e, "nameProblem").visible);
        }

        function test_draft_json_shape() {
            const e = make();
            e.authorField.text = "Sam";
            const draft = JSON.parse(e.draftJson());
            compare(Object.keys(draft).sort(), ["author", "dark", "light", "name"]);
            compare(draft.name, "Gruvbox copy");
            compare(draft.author, "Sam");
            const keys = Fixture.editor.keys.map(k => k.key).sort();
            compare(Object.keys(draft.dark).sort(), keys);
            compare(Object.keys(draft.light).sort(), keys);
            compare(draft.dark.bg, "#282828");
        }

        function test_short_hex_expanded_by_logic_not_qml() {
            const e = make();
            type(e, "light-accent", "#abc");
            compare(JSON.parse(e.draftJson()).light.accent, "#abc");
            compare(control(e, "light-accent").text, "#abc");
        }

        function test_save_sends_the_draft_and_waits_for_uqd() {
            const e = make();
            const spy = createTemporaryObject(spyComp, root, { target: e, signalName: "saved" });
            e.save();
            compare(spy.count, 1);
            compare(JSON.parse(spy.signalArguments[0][0]).name, "Gruvbox copy");
            verify(e.saving);
            e.finishSave("It didn't work");
            verify(!e.saving);
            compare(e.saveError, "That wasn't saved: It didn't work");
            verify(control(e, "saveError").visible);
            compare(control(e, "saveError").textFormat, Text.PlainText);
        }

        function test_swatch_has_a_name() {
            const e = make();
            compare(control(e, "swatch-dark-bg").Accessible.name, "Background colour");
        }
    }
}
