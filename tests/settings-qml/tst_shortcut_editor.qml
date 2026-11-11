import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"
import "fixtures/keybinds_catalogue.js" as Fixture

Item {
    id: root
    width: 900
    height: 800

    property var asked: []

    // KeybindsModel.check for the tests: Super + Space is Spotlight's, Super + T is a marked
    // line of user.lua; every other combo is free.
    function check(draftJson) {
        const d = JSON.parse(draftJson);
        root.asked.push(d);
        if (d.combo === "SUPER + Space" && d.id !== "uq.spotlight") {
            return JSON.stringify({
                valid: false,
                error: "“" + (d.name || "Open Spotlight") + "” and “Open Spotlight” both use Super + Space.",
                conflicts: [{ id: "uq.spotlight", label: "Open Spotlight" }],
                user_lua: [],
                combo_text: "Super + Space"
            });
        }
        return JSON.stringify({
            valid: true,
            error: "",
            conflicts: [],
            user_lua: d.combo === "SUPER + T" ? [{ line: 7, text: "hl.bind(\"SUPER + T\", …)" }] : [],
            combo_text: d.combo
        });
    }

    Component {
        id: comp
        ShortcutEditor {
            catalogue: Fixture.catalogue
            apps: [{ id: "firefox.desktop", name: "Firefox", icon: "firefox" }, { id: "foot.desktop", name: "Foot", icon: "foot" }]
            limits: ({ name: 100, command: 1024, custom: 100 })
            check: root.check
            comboFrom: (key, modifiers) => key === Qt.Key_Q ? "SUPER + Q" : ""
        }
    }
    Component {
        id: spyComp
        SignalSpy {}
    }

    TestCase {
        name: "ShortcutEditor"
        when: windowShown

        // An editor that is open, as the person sees it, with `row` loaded.
        function make(row, custom) {
            const e = createTemporaryObject(comp, root);
            verify(e);
            e.load(row === undefined ? null : row, custom === undefined ? true : custom);
            e.open();
            tryVerify(() => e.opened);
            return e;
        }
        function control(e, name) {
            const c = findChild(e.body, name);
            verify(c, "no control " + name);
            return c;
        }
        function spotlightRow() {
            return { id: "uq.spotlight", label: "Open Spotlight", combo: "SUPER + Space", combo_text: "Super + Space", enabled: true };
        }
        function fillNew(e, name, combo) {
            e.nameField.text = name;
            e.nameField.textEdited();
            e.setFromCombo(combo);
        }

        function test_conflict_warning_shows_save_is_off_and_clears() {
            const e = make();
            verify(!control(e, "conflictWarning").visible, "nothing to warn about yet");
            fillNew(e, "Mine", "SUPER + Space");
            verify(control(e, "conflictWarning").visible);
            compare(control(e, "conflictWarning").text, "“Mine” and “Open Spotlight” both use Super + Space.");
            verify(!e.saveEnabled, "Save is disabled while the keys are taken");
            // Changing the keys clears the warning.
            e.setFromCombo("SUPER + K");
            verify(!control(e, "conflictWarning").visible);
            verify(e.saveEnabled);
        }

        function test_warning_is_plain_text() {
            const e = make();
            const w = control(e, "conflictWarning");
            compare(w.textFormat, Text.PlainText);
        }

        function test_marked_user_lua_warns_but_still_saves() {
            const e = make();
            fillNew(e, "Terminal", "SUPER + T");
            verify(control(e, "userLuaWarning").visible);
            compare(control(e, "userLuaWarning").text, "A shortcut in user.lua, line 7, uses these keys too.");
            verify(e.saveEnabled);
        }

        function test_modifier_switches_and_key_list_build_the_combo() {
            const e = make();
            compare(e.combo, "", "no key yet");
            control(e, "mod_SUPER").toggle();
            control(e, "mod_SUPER").toggled();
            control(e, "mod_SHIFT").toggle();
            control(e, "mod_SHIFT").toggled();
            const picker = control(e, "keyPicker");
            const at = e.keyChoices.findIndex(k => k.spelling === "K");
            verify(at >= 0);
            picker.currentIndex = at;
            picker.activated(at);
            compare(e.combo, "SUPER + SHIFT + K");
            control(e, "mod_SHIFT").toggle();
            control(e, "mod_SHIFT").toggled();
            compare(e.combo, "SUPER + K");
        }

        function test_capture_fills_the_keys() {
            const e = make();
            const started = createTemporaryObject(spyComp, root, { target: e, signalName: "captureStarted" });
            const ended = createTemporaryObject(spyComp, root, { target: e, signalName: "captureEnded" });
            e.capture.start();
            compare(started.count, 1);
            keyPress(Qt.Key_Q, Qt.MetaModifier);
            compare(e.combo, "SUPER + Q");
            verify(control(e, "mod_SUPER").checked);
            compare(ended.count, 1);
            compare(control(e, "comboWords").text, "SUPER + Q");
        }

        function test_closing_ends_a_capture() {
            const e = make();
            const ended = createTemporaryObject(spyComp, root, { target: e, signalName: "captureEnded" });
            e.capture.start();
            e.close();
            tryVerify(() => !e.opened);
            compare(ended.count, 1);
        }

        function test_capture_hidden_when_pausing_does_not_work() {
            const e = make();
            e.captureAvailable = false;
            verify(!e.capture.visible);
        }

        function test_new_shortcut_builds_json_for_each_kind() {
            const e = make();
            fillNew(e, "  Spotlight too ", "SUPER + F9");
            let b = JSON.parse(JSON.stringify(e.draft()));
            compare(b.name, "Spotlight too");
            compare(b.combo, "SUPER + F9");
            compare(b.action, { kind: "uq", name: "terminal" });
            verify(!("id" in b));
            // Start an app.
            const kind = control(e, "actionKind");
            kind.currentIndex = 4;
            kind.activated(4);
            verify(!e.saveEnabled, "an app must be chosen");
            const app = control(e, "appChoice");
            app.currentIndex = 1;
            app.activated(1);
            compare(e.draft().action, { kind: "launch", app: "foot.desktop" });
            verify(e.saveEnabled);
            // Run a command.
            kind.currentIndex = 5;
            kind.activated(5);
            verify(!e.saveEnabled, "a command must be typed");
            e.commandField.text = "  foot --title notes ";
            e.commandField.textEdited();
            compare(e.draft().action, { kind: "command", command: "foot --title notes" });
            verify(e.saveEnabled);
            // A uQ panel.
            kind.currentIndex = 0;
            kind.activated(0);
            compare(e.draft().action, { kind: "shell", name: "spotlight" });
        }

        function test_lock_and_repeat_switches_only_where_allowed() {
            const e = make();
            const kind = control(e, "actionKind");
            const choice = control(e, "actionChoice");
            kind.currentIndex = 0;
            kind.activated(0);
            // Spotlight: no lock screen. The volume keys: both.
            choice.currentIndex = 0;
            choice.activated(0);
            verify(!control(e, "lockedSwitch").visible);
            choice.currentIndex = 1;
            choice.activated(1);
            verify(control(e, "lockedSwitch").visible);
            verify(control(e, "repeatSwitch").visible);
            control(e, "lockedSwitch").checked = true;
            fillNew(e, "Louder", "SUPER + F9");
            compare(e.draft().locked, true);
            // A command never is, even if the switch was left on.
            kind.currentIndex = 5;
            kind.activated(5);
            verify(!control(e, "lockedSwitch").visible);
            verify(!control(e, "repeatSwitch").visible);
            compare(e.draft().locked, false);
            compare(e.draft().repeating, false);
        }

        function test_changing_one_of_uqs_edits_only_the_keys() {
            const e = make(spotlightRow(), false);
            verify(!e.custom);
            verify(!control(e, "shortcutName").visible);
            verify(!control(e, "actionKind").visible);
            compare(e.combo, "SUPER + Space");
            verify(e.saveEnabled, "its own keys are no conflict");
            e.setFromCombo("SUPER + K");
            const accepted = createTemporaryObject(spyComp, root, { target: e, signalName: "comboAccepted" });
            e.save();
            compare(accepted.count, 1);
            compare(accepted.signalArguments[0][0], "uq.spotlight");
            compare(accepted.signalArguments[0][1], "SUPER + K");
            verify(e.saving);
            verify(!e.saveEnabled, "not twice");
        }

        function test_a_mouse_button_default_only_offers_mouse_buttons() {
            const e = make({ id: "uq.move", label: "Move a window", combo: "SUPER + mouse_left", combo_text: "Super + Left mouse button", enabled: true }, false);
            // Only mouse buttons are offered, and a keyboard can't press one.
            verify(e.keyChoices.length > 0);
            verify(e.keyChoices.every(k => k.kind === "button"));
            verify(!e.capture.available, "a mouse button can't be pressed on the keyboard");
        }

        function test_editing_your_own_loads_its_action() {
            const e = make({ id: "k-3f9c", name: "Louder", label: "Louder", combo: "SUPER + F9", combo_text: "Super + F9", enabled: true,
                raw: { id: "k-3f9c", name: "Louder", combo: "SUPER + F9", enabled: true, locked: true, repeating: false, action: { kind: "shell", name: "vol-up" } } }, true);
            compare(e.nameField.text, "Louder");
            compare(e.kind, "shell");
            compare(e.currentEntry.label, "Turns the volume up");
            compare(e.draft().id, "k-3f9c");
            compare(e.draft().locked, true);
            const accepted = createTemporaryObject(spyComp, root, { target: e, signalName: "bindAccepted" });
            e.save();
            compare(accepted.count, 1);
            compare(JSON.parse(accepted.signalArguments[0][0]).id, "k-3f9c");
        }

        function test_save_stays_open_until_uqd_answers() {
            const e = make();
            fillNew(e, "Mine", "SUPER + F9");
            e.errorNow = () => "<i>no</i> such thing";
            e.busy = false;
            e.save();
            verify(e.saving);
            e.busy = true;
            e.busy = false;
            tryCompare(e, "saving", false);
            compare(e.saveError, "That wasn't saved: <i>no</i> such thing", "uQ's reason is kept, as text");
            compare(control(e, "saveError").textFormat, Text.PlainText);
            verify(e.saveEnabled, "it can be tried again");
        }

        function test_a_name_is_needed() {
            const e = make();
            e.setFromCombo("SUPER + F9");
            verify(!e.saveEnabled);
            fillNew(e, "Mine", "SUPER + F9");
            verify(e.saveEnabled);
            e.canSave = false;
            verify(!e.saveEnabled, "not while a request is in flight");
        }

        function test_the_title_is_a_fixed_phrase() {
            const e = make({ id: "k-1", name: "<b>x</b>", raw: { name: "<b>x</b>", combo: "SUPER + F9", action: { kind: "uq", name: "terminal" } } }, true);
            compare(e.title, "Change shortcut");
            const n = make();
            compare(n.title, "New shortcut");
        }
    }
}
