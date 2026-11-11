import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"
import "fixtures/compat_fields.js" as Fixture

Item {
    id: root
    width: 900
    height: 800

    Component {
        id: comp
        AppFixDialog {
            fields: Fixture.fields
            apps: [{ id: "code.desktop", name: "Code", icon: "" }, { id: "steam.desktop", name: "Steam", icon: "" }]
        }
    }

    Component {
        id: spyComp
        SignalSpy {}
    }

    TestCase {
        name: "AppFixDialog"
        when: windowShown

        function make() {
            const d = createTemporaryObject(comp, root);
            verify(d);
            return d;
        }
        function control(d, name) {
            const c = findChild(d.body, name);
            verify(c, "no control " + name);
            return c;
        }
        function choose(combo, index) {
            combo.currentIndex = index;
            combo.activated(index);
        }

        function test_builds_fix_json_from_fields() {
            const d = make();
            d.start("code.desktop", "Code", null);
            compare(JSON.parse(d.fixJson()), { id: "code.desktop", platform: "auto", scale: null, env: [], args: [], elsewhere: false });
            choose(d.platformBox, 1);
            choose(d.scaleBox, 3);
            d.addVar();
            d.envList[0] = { name: "GTK_THEME", value: "Adwaita:dark" };
            d.argsBox.text = "--enable-features=X\n\n  --disable-gpu  ";
            d.elsewhereBox.checked = true;
            compare(JSON.parse(d.fixJson()), {
                id: "code.desktop",
                platform: Fixture.fields.platforms[1].key,
                scale: Fixture.fields.scales[2],
                env: [{ name: "GTK_THEME", value: "Adwaita:dark" }],
                args: ["--enable-features=X", "--disable-gpu"],
                elsewhere: true
            });
            // Editing an existing fix fills every field from it.
            const fix = { id: "steam.desktop", platform: "x11", scale: 2.0, env: [{ name: "A", value: "1" }], args: ["-x"], elsewhere: true };
            d.start("steam.desktop", "Steam", fix);
            compare(JSON.parse(d.fixJson()), fix);
        }

        function test_title_is_fixed() {
            const d = make();
            compare(d.title, "Fix this app");
            d.start("x.desktop", "<b>x</b>", null);
            compare(d.title, "Fix this app");
            const name = control(d, "fixAppName");
            compare(name.text, "<b>x</b>");
            compare(name.textFormat, Text.PlainText);
            d.startChoosing("<i>window</i>");
            compare(d.title, "Fix this app");
            verify(control(d, "chooseHint").visible);
            compare(control(d, "chooseHint").text, "uQ couldn't tell which app this is. Choose it:");
            choose(control(d, "appChooser"), 1);
            compare(d.appId, "steam.desktop");
        }

        function test_scale_steps() {
            const d = make();
            compare(d.scaleLabels, ["Automatic", "100 %", "125 %", "150 %", "175 %", "200 %", "225 %", "250 %", "275 %", "300 %"]);
            d.start("code.desktop", "Code", null);
            for (let i = 1; i < d.scaleLabels.length; i++) {
                choose(d.scaleBox, i);
                compare(JSON.parse(d.fixJson()).scale, 1 + (i - 1) * 0.25);
            }
            choose(d.scaleBox, 0);
            compare(JSON.parse(d.fixJson()).scale, null);
        }

        function test_elsewhere_sentence_shown() {
            const d = make();
            d.start("code.desktop", "Code", null);
            const sentence = control(d, "elsewhereSentence");
            compare(sentence.text, "uQ saves its own copy of this app's launcher file, and removes it when you remove the fix.");
            compare(control(d, "elsewhere").text, "Also when started from other launchers");
        }

        function test_save_waits_for_the_answer() {
            const d = make();
            d.preview = json => JSON.stringify({ env: ["GDK_BACKEND"], args: [], problem: null });
            d.start("code.desktop", "Code", null);
            choose(d.platformBox, 1);
            d.runPreview();
            verify(d.saveEnabled);
            const spy = createTemporaryObject(spyComp, root, { target: d, signalName: "fixAccepted" });
            d.save();
            compare(spy.count, 1);
            compare(JSON.parse(spy.signalArguments[0][0]).platform, Fixture.fields.platforms[1].key);
            verify(d.saving);
            d.errorNow = () => "<b>no</b>";
            d.busy = true;
            d.busy = false;
            tryCompare(d, "saving", false);
            compare(d.saveError, "That wasn't saved: <b>no</b>");
            compare(control(d, "saveError").textFormat, Text.PlainText);
            d.preview = json => JSON.stringify({ env: [], args: [], problem: "This fix doesn't change anything." });
            d.runPreview();
            verify(!d.saveEnabled, "a fix uQ would refuse can't be saved");
        }
    }
}
