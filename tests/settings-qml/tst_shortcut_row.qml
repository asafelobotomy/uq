import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 900
    height: 200

    Component {
        id: comp
        ShortcutRow {
            width: 880
            opensFiles: false
        }
    }
    Component {
        id: spyComp
        SignalSpy {}
    }

    function sample(extra) {
        return Object.assign({
            id: "uq.spotlight",
            label: "Open Spotlight",
            combo: "SUPER + Space",
            combo_text: "Super + Space",
            default_combo_text: "Super + Space",
            enabled: true,
            is_default: true,
            changed: false,
            can_disable: true,
            action_text: "Open Spotlight",
            source: "default",
            line: null,
            path: null,
            profile: ""
        }, extra || {});
    }

    TestCase {
        name: "ShortcutRow"
        when: windowShown

        function make(bind) {
            const r = createTemporaryObject(comp, root, { bind: bind });
            verify(r);
            return r;
        }
        function child(r, name) {
            const c = findChild(r, name);
            verify(c, "no " + name);
            return c;
        }

        function test_shows_name_and_keys_in_words() {
            const r = make(root.sample());
            compare(r.title, "Open Spotlight");
            compare(child(r, "comboText").text, "Super + Space");
            verify(!child(r, "reset").visible, "Reset only for a changed shortcut");
            verify(!child(r, "delete").visible);
            verify(!child(r, "actionText").visible, "a default's name already says what it does");
            verify(!child(r, "alwaysOn").visible);
            verify(!child(r, "shortcutProfile").visible);
            compare(r.sourceText, "", "an untouched default says nothing under its name");
        }

        function test_changed_shows_reset() {
            const r = make(root.sample({ combo: "SUPER + K", combo_text: "Super + K", changed: true, is_default: false, source: "uq" }));
            compare(child(r, "comboText").text, "Super + K");
            verify(child(r, "reset").visible);
            const spy = createTemporaryObject(spyComp, root, { target: r, signalName: "resetRequested" });
            child(r, "reset").clicked();
            compare(spy.count, 1);
        }

        function test_change_button_asks_for_the_editor() {
            const r = make(root.sample());
            const spy = createTemporaryObject(spyComp, root, { target: r, signalName: "changeRequested" });
            child(r, "change").clicked();
            compare(spy.count, 1);
            r.idle = false;
            verify(!child(r, "change").enabled);
        }

        function test_switch_reports_and_snaps_back() {
            const r = make(root.sample());
            const spy = createTemporaryObject(spyComp, root, { target: r, signalName: "switched" });
            const sw = r.toggle;
            verify(sw.checked);
            sw.toggle();
            sw.toggled();
            compare(spy.count, 1);
            compare(spy.signalArguments[0][0], false);
            verify(sw.checked, "the switch shows the shortcut again until uQ confirms");
            r.bind = root.sample({ enabled: false });
            verify(!sw.checked);
            verify(child(r, "comboText").opacity < 1, "a shortcut that is off is dimmed");
        }

        function test_lock_recovery_switch_is_disabled_with_a_reason() {
            const r = make(root.sample({ id: "uq.lock-recovery", label: "Get back in if the lock screen stops", can_disable: false }));
            verify(!r.toggle.enabled);
            verify(child(r, "alwaysOn").visible);
            verify(child(r, "alwaysOn").text.indexOf("Always on") === 0);
            verify(r.toggle.Accessible.description.indexOf("Always on") >= 0, "a screen reader hears why");
        }

        function test_profile_line_under_the_switch() {
            const r = make(root.sample({ profile: "Gaming" }));
            verify(child(r, "shortcutProfile").visible);
            compare(child(r, "shortcutProfile").text, "From the Gaming profile");
            compare(r.sourceText, "", "said once, under the switch");
        }

        function test_user_lua_override_is_said_under_the_name() {
            const r = make(root.sample({ source: "user_lua", line: 12, path: "/home/me/user.lua" }));
            compare(r.sourceText, "Overridden in user.lua, line 12");
            verify(child(r, "openUserLuaButton").visible);
        }

        function test_a_refused_change_is_not_repeated_on_every_row() {
            const r = make(root.sample({ source: "error" }));
            compare(r.sourceText, "");
        }

        function test_your_own_shortcut_has_name_action_and_delete() {
            const r = make(root.sample({ id: "k-3f9c", label: "Open my terminal", name: "Open my terminal", action_text: "Opens a terminal", can_disable: true, is_default: false, source: "uq" }));
            compare(r.title, "Open my terminal");
            verify(r.custom);
            verify(child(r, "actionText").visible);
            compare(child(r, "actionText").text, "Opens a terminal");
            verify(child(r, "delete").visible);
            verify(!child(r, "reset").visible);
            const spy = createTemporaryObject(spyComp, root, { target: r, signalName: "deleteRequested" });
            child(r, "delete").clicked();
            compare(spy.count, 1);
        }

        function test_keys_are_drawn_as_keycaps() {
            const r = make(root.sample({ combo_text: "Super + Shift + Q" }));
            const caps = child(r, "keycaps");
            verify(caps.visible);
            compare(caps.keys, ["Super", "Shift", "Q"]);
            verify(!child(r, "comboText").visible, "the keycaps say it; the words are for no keys");
            r.bind = root.sample({ combo: null, combo_text: null });
            verify(!caps.visible);
            verify(child(r, "comboText").visible);
        }

        function test_no_keys_is_said_plainly() {
            const r = make(root.sample({ id: "uq.profile.next", label: "Switch to the next profile", combo: null, combo_text: null }));
            compare(child(r, "comboText").text, "No keys");
        }

        function test_markup_in_a_name_shows_literally() {
            const r = make(root.sample({ id: "k-1", name: "<b>Mine</b>", label: "<b>Mine</b>", action_text: "<i>x</i>" }));
            const t = child(r, "comboText");
            compare(t.textFormat, Text.PlainText);
            compare(child(r, "actionText").text, "<i>x</i>");
            compare(child(r, "actionText").textFormat, Text.PlainText);
        }
    }
}
