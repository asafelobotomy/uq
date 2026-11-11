import QtQuick
import QtTest
import "../../shell/logic/greeter.js" as G

// shell/logic/greeter.js: everything uq-greeter decides (spec phase 5 §2).
TestCase {
    name: "greeter_logic"

    // --- files ---
    function test_toml_reader() {
        var d = G.parseToml('# c\ntheme = "forest" # trailing\nhide_users = true\nn = -3\n'
                            + 'bad line\nx = "unterminated\n[sessions]\n"al ice" = "a\\"b"\nbob = "uq.desktop"\n');
        compare(d[""].theme, "forest");
        compare(d[""].hide_users, true);
        compare(d[""].n, -3);
        compare(d[""].x, undefined);
        compare(d.sessions["al ice"], 'a"b');
        compare(d.sessions.bob, "uq.desktop");
        compare(G.parseToml(null)[""], {});
        compare(G.parseToml("[bad\nk = 1\n")[""].k, undefined);
    }

    function test_greeter_config_defaults_and_validation() {
        compare(G.greeterConfig(""), { theme: "graphite", variant: "dark", hideUsers: false, reduceMotion: false });
        compare(G.greeterConfig(null).theme, "graphite");
        compare(G.greeterConfig('theme = "rosewood"\nvariant = "light"\nhide_users = true\nreduce_motion = true\n'),
                { theme: "rosewood", variant: "light", hideUsers: true, reduceMotion: true });
        // A theme name can't walk out of the rendered directory; a bad variant falls back.
        compare(G.greeterConfig('theme = "../../etc"\nvariant = "dim"\n'),
                { theme: "graphite", variant: "dark", hideUsers: false, reduceMotion: false });
        compare(G.greeterConfig('hide_users = "yes"\n').hideUsers, false);
    }

    function test_tokens_path() {
        compare(G.tokensPath({ theme: "forest", variant: "light" }),
                "/usr/share/uq/themes/rendered/forest-light/tokens.json");
        compare(G.tokensPath({ theme: "forest", variant: "light" }, true),
                "/usr/share/uq/themes/rendered/graphite-dark/tokens.json");
        compare(G.tokensPath(null), "/usr/share/uq/themes/rendered/graphite-dark/tokens.json");
    }

    function test_state_round_trip() {
        var st = G.recordLaunch(G.parseState(""), "alice", "uq.desktop", 1000);
        st = G.recordLaunch(st, "bob", "sway.desktop", 2000);
        var back = G.parseState(G.serializeState(st));
        compare(back.lastUser, "bob");
        compare(back.sessions, { alice: "uq.desktop", bob: "sway.desktop" });
        compare(back.launchedAt, 2000);
        compare(back.launchedUser, "bob");
        // Bad entries are dropped on read.
        var bad = G.parseState('last_user = "x y"\n[sessions]\n"b\\"ad" = "uq.desktop"\nok = "../x.desktop"\ncarol = "uq.desktop"\n');
        compare(bad.lastUser, "");
        compare(bad.sessions, { carol: "uq.desktop" });
        compare(G.tomlString('a"\\\n'), '"a\\"\\\\\\u000a"');
    }

    function test_ended_early() {
        var st = G.recordLaunch(G.parseState(""), "alice", "uq.desktop", 100000);
        compare(G.endedEarly(st, 100000 + 3000), "alice");
        compare(G.endedEarly(st, 100000 + G.ENDED_EARLY_MS + 1), "");
        compare(G.endedEarly(st, 50000), "");  // clock went back: not "early"
        compare(G.endedEarly(G.clearLaunch(st), 100001), "");
        compare(G.clearLaunch(st).lastUser, "alice");
        compare(G.endedEarly(G.parseState(""), 5), "");
    }

    // --- sessions ---
    function test_desktop_entry() {
        var e = G.parseDesktopEntry("[Desktop Entry]\nName=Sway\nName[de]=Schwanken\nExec=sway --unsupported-gpu\nType=Application\n[Other]\nExec=evil\n");
        compare(e, { name: "Sway", exec: "sway --unsupported-gpu" });
        compare(G.parseDesktopEntry("[Desktop Entry]\nName=X\n"), null);
        compare(G.parseDesktopEntry("[Other]\nName=X\nExec=x\n"), null);
        compare(G.parseDesktopEntry("[Desktop Entry]\nName=A\\sB\nExec=a\\\\b\n"), { name: "A B", exec: "a\\b" });
        compare(G.parseDesktopEntry("[Desktop Entry]\nName=<b>x</b>\nExec=x\n").name, "<b>x</b>");
    }

    function test_split_exec() {
        compare(G.splitExec("Hyprland"), ["Hyprland"]);
        compare(G.splitExec("  sway   --flag  "), ["sway", "--flag"]);
        compare(G.splitExec('"/opt/my app/run" --name "a b"'), ["/opt/my app/run", "--name", "a b"]);
        compare(G.splitExec('sh -c "echo \\"hi\\" \\$HOME \\`x\\` \\\\"'), ["sh", "-c", 'echo "hi" $HOME `x` \\']);
        compare(G.splitExec("app %U --x=%f 100%%"), ["app", "--x=", "100%"]);
        compare(G.splitExec('"unterminated'), null);
        compare(G.splitExec(""), null);
        compare(G.splitExec('""'), null);
        compare(G.splitExec(null), null);
        // Desktop-string escapes are undone before the quoting rules (four backslashes → one).
        compare(G.splitExec(G.parseDesktopEntry('[Desktop Entry]\nName=a\nExec="a\\\\\\\\b"\n').exec), ["a\\b"]);
    }

    function test_session_list_puts_uq_first() {
        var files = G.parseSessionDump(
            "\u001esway.desktop\u001f[Desktop Entry]\nName=Sway\nExec=sway\n"
            + "\u001ehyprland.desktop\u001f[Desktop Entry]\nName=Hyprland\nExec=Hyprland\n"
            + "\u001euq.desktop\u001f[Desktop Entry]\nName=uQ\nExec=/usr/bin/uq-session\n"
            + "\u001ebroken.desktop\u001f[Desktop Entry]\nName=Broken\n"
            + "\u001e.hidden.desktop\u001f[Desktop Entry]\nName=H\nExec=h\n"
            + "\u001enot-a-desktop.txt\u001f[Desktop Entry]\nName=T\nExec=t\n");
        var list = G.sessionList(files);
        compare(list.map(function (s) { return s.id; }), ["uq.desktop", "hyprland.desktop", "sway.desktop"]);
        compare(list[0].command, ["/usr/bin/uq-session"]);
        compare(G.isUqSession(list[0]), true);
        compare(G.sessionList([]), []);
        compare(G.sessionList(null), []);
    }

    function test_session_preselection() {
        var list = [{ id: "uq.desktop" }, { id: "sway.desktop" }];
        var st = G.parseState('[sessions]\nbob = "sway.desktop"\ncarol = "gone.desktop"\n');
        compare(G.sessionIndexFor(list, st, "bob"), 1);
        compare(G.sessionIndexFor(list, st, "carol"), 0);
        compare(G.sessionIndexFor(list, st, "alice"), 0);
        compare(G.sessionIndexFor([], st, "bob"), -1);
        compare(G.launchEnv(), ["XDG_SESSION_TYPE=wayland"]);
    }

    // --- users ---
    function test_user_paths() {
        compare(G.parseUserPaths('{"type":"ao","data":[["/org/freedesktop/Accounts/User1000","/org/freedesktop/Accounts/User1001","/evil"]]}'),
                ["/org/freedesktop/Accounts/User1000", "/org/freedesktop/Accounts/User1001"]);
        compare(G.parseUserPaths("not json"), []);
        compare(G.parseUserPaths('{"type":"ao","data":[]}'), []);
    }

    function props(name, real, icon, system, locked) {
        return JSON.stringify({ type: "a{sv}", data: [{
            UserName: { type: "s", data: name }, RealName: { type: "s", data: real },
            IconFile: { type: "s", data: icon }, SystemAccount: { type: "b", data: system },
            Locked: { type: "b", data: locked }, Uid: { type: "t", data: 1000 } }] });
    }

    function test_user_props_and_filtering() {
        var a = G.parseUserProps(props("alice", "Alice\nLiddell", "/var/lib/AccountsService/icons/alice", false, false));
        compare(a, { name: "alice", realName: "Alice Liddell", icon: "/var/lib/AccountsService/icons/alice",
                     system: false, locked: false });
        // Avatars only from AccountsService's own icon folder.
        compare(G.parseUserProps(props("bob", "", "/home/bob/.face", false, false)).icon, "");
        compare(G.parseUserProps(props("bob", "", "/var/lib/AccountsService/icons/../../../etc/shadow", false, false)).icon, "");
        compare(G.parseUserProps(props("bad name", "", "", false, false)), null);
        compare(G.parseUserProps("{}"), null);
        var users = G.visibleUsers([
            G.parseUserProps(props("zed", "", "", false, false)),
            a,
            G.parseUserProps(props("sddm", "", "", true, false)),
            G.parseUserProps(props("old", "Old", "", false, true)),
            a, null
        ]);
        compare(users.map(function (u) { return u.name; }), ["alice", "zed"]);
        compare(users[0].label, "Alice Liddell");
        compare(users[0].avatar.path, "file:///var/lib/AccountsService/icons/alice");
        compare(users[1].avatar.initial, "Z");
    }

    function test_initial_user() {
        var users = [{ name: "alice" }, { name: "bob" }];
        compare(G.initialUser(users, G.parseState('last_user = "bob"\n')), "bob");
        compare(G.initialUser(users, G.parseState('last_user = "gone"\n')), "alice");
        compare(G.initialUser(users, G.parseState('last_user = "bob"\n'), "alice"), "alice");
        compare(G.initialUser([], G.parseState("")), "");
    }

    // --- conversation ---
    function step(r, actions) {
        if (actions !== undefined)
            compare(r.actions, actions);
        return r.conv;
    }

    function test_password_login() {
        var c = G.newConversation();
        c = step(G.submit(c, "alice", "hunter2"), [{ create: "alice" }]);
        compare(c.phase, "starting");
        compare(G.fieldMode(c), "busy");
        c = step(G.onAuthMessage(c, "Password: ", false, true, false), [{ respond: "hunter2" }]);
        compare(c.pending, null);
        c = step(G.onReady(c), [{ launch: true }]);
        compare(c.phase, "launching");
        // Too late to cancel once launching.
        compare(G.cancel(c, "bob").actions, []);
        c = step(G.onLaunched(c), []);
        compare(c.phase, "launched");
    }

    function test_wrong_password_clears_and_shows_failure() {
        var c = step(G.submit(G.newConversation(), "alice", "nope"));
        c = step(G.onAuthMessage(c, "Password:", false, true, false), [{ respond: "nope" }]);
        c = step(G.onAuthFailure(c, "pam_authenticate: AUTH_ERR"), []);
        compare(c.phase, "idle");
        compare(c.user, "alice");
        compare(c.message, { text: "Authentication failed", error: true });
        compare(G.fieldMode(c), "password");
    }

    function test_faillock_messages_are_shown() {
        var c = step(G.submit(G.newConversation(), "alice", "pw"));
        c = step(G.onAuthMessage(c, "The account is locked due to 3 failed logins.", true, false, false), []);
        compare(c.message, { text: "The account is locked due to 3 failed logins.", error: true });
        c = step(G.onAuthMessage(c, "(10 minutes left to unlock)", false, false, false), []);
        c = step(G.onAuthFailure(c, "pam_authenticate: MAXTRIES"));
        compare(c.message, { text: "The account is locked due to 3 failed logins. (10 minutes left to unlock)", error: true });
        compare(c.pending, null);
    }

    function test_empty_submit_waits_for_the_prompt() {
        var c = step(G.submit(G.newConversation(), "alice", ""), [{ create: "alice" }]);
        compare(c.pending, null);
        c = step(G.onAuthMessage(c, "Password: ", false, true, false), []);
        compare(G.fieldMode(c), "secret");
        compare(c.prompt.text, "Password");
        c = step(G.submit(c, "alice", "typed"), [{ respond: "typed" }]);
        compare(c.phase, "waiting");
    }

    function test_echo_on_prompt_is_a_visible_field() {
        var c = step(G.submit(G.newConversation(), "alice", "pw"));
        c = step(G.onAuthMessage(c, "Verification code:‮", false, true, true), []);
        compare(G.fieldMode(c), "visible");
        compare(c.prompt.text, "Verification code");
        compare(c.pending, "pw");  // kept for a later echo-off prompt
        c = step(G.answer(c, "123456"), [{ respond: "123456" }]);
        c = step(G.onAuthMessage(c, "Password:", false, true, false), [{ respond: "pw" }]);
    }

    function test_info_message_then_prompt() {
        var c = step(G.submit(G.newConversation(), "alice", "pw"));
        c = step(G.onAuthMessage(c, "Place your finger on the reader", false, false, false), []);
        compare(c.message, { text: "Place your finger on the reader", error: false });
        compare(c.phase, "starting");
        c = step(G.onAuthMessage(c, "Password:", false, true, false), [{ respond: "pw" }]);
    }

    function test_expired_password() {
        var c = step(G.submit(G.newConversation(), "alice", "old"));
        c = step(G.onAuthMessage(c, "Password: ", false, true, false), [{ respond: "old" }]);
        c = step(G.onAuthMessage(c, "You are required to change your password immediately (administrator enforced).", true, false, false), []);
        c = step(G.onAuthMessage(c, "Changing password for alice.", false, false, false), []);
        // pam_unix asks for the current password again: the user types it.
        c = step(G.onAuthMessage(c, "Current password: ", false, true, false), []);
        compare(G.fieldMode(c), "secret");
        compare(c.prompt.text, "Current password");
        c = step(G.answer(c, "old"), [{ respond: "old" }]);
        c = step(G.onAuthMessage(c, "New password: ", false, true, false), []);
        compare(G.fieldMode(c), "new");
        // A mismatch is caught here and never sent.
        var mm = G.answerNew(c, "abc123", "abc124");
        compare(mm.actions, []);
        compare(mm.conv.message, { text: "The new passwords don't match", error: true });
        compare(G.fieldMode(mm.conv), "new");
        compare(G.answerNew(c, "", "").actions, []);
        // Main-field Enter can't answer the new-password form.
        compare(G.submit(c, "alice", "x").actions, []);
        c = step(G.answerNew(mm.conv, "abc123", "abc123"), [{ respond: "abc123" }]);
        c = step(G.onAuthMessage(c, "Retype new password: ", false, true, false), [{ respond: "abc123" }]);
        compare(c.confirm, null);
        c = step(G.onReady(c), [{ launch: true }]);
    }

    function test_expired_password_rejected_by_pam() {
        var c = step(G.submit(G.newConversation(), "alice", "old"));
        c = step(G.onAuthMessage(c, "Password:", false, true, false));
        c = step(G.onAuthMessage(c, "New password:", false, true, false));
        c = step(G.answerNew(c, "short", "short"), [{ respond: "short" }]);
        c = step(G.onAuthMessage(c, "BAD PASSWORD: The password is shorter than 8 characters", true, false, false));
        // PAM asks again: a fresh form.
        c = step(G.onAuthMessage(c, "New password:", false, true, false), []);
        compare(G.fieldMode(c), "new");
        compare(c.message.text, "BAD PASSWORD: The password is shorter than 8 characters");
    }

    function test_classify_prompt() {
        compare(G.classifyPrompt("Password: "), "secret");
        compare(G.classifyPrompt("Current password: "), "secret");
        compare(G.classifyPrompt("New password: "), "new");
        compare(G.classifyPrompt("Enter new UNIX password: "), "new");
        compare(G.classifyPrompt("Retype new password: "), "confirm");
        compare(G.classifyPrompt("Re-enter new password:"), "confirm");
        compare(G.classifyPrompt("New password (again):"), "confirm");
    }

    function test_cancel() {
        var c = step(G.submit(G.newConversation(), "alice", "pw"));
        var r = G.cancel(c, "bob");
        compare(r.actions, [{ cancel: true }]);
        compare(r.conv.user, "bob");
        compare(r.conv.pending, null);
        compare(r.conv.phase, "idle");
        compare(G.cancel(G.newConversation(), "bob").actions, []);
        // A prompt waiting on the user is cancelled too.
        c = step(G.onAuthMessage(step(G.submit(G.newConversation(), "alice", "")), "Password:", false, true, false));
        compare(G.cancel(c, "").actions, [{ cancel: true }]);
        compare(G.cancel(c, "").conv.user, "alice");
    }

    function test_greetd_error_and_stray_events() {
        var c = step(G.submit(G.newConversation(), "alice", "pw"));
        c = step(G.onError(c, "some internal error"), []);
        compare(c.message, { text: "The login service reported a problem", error: true });
        compare(c.phase, "idle");
        // Messages with no conversation are ignored.
        compare(G.onAuthMessage(c, "Password:", false, true, false).actions, []);
        compare(G.onReady(c).actions, []);
        // A second submit while one is in flight does nothing.
        var busy = step(G.submit(G.newConversation(), "alice", "pw"));
        compare(G.submit(busy, "alice", "again").actions, []);
        compare(G.submit(G.newConversation(), "bad user", "pw").actions, []);
    }

    // --- power ---
    function test_power_needs_a_second_click() {
        var r = G.powerClick(null, "reboot", 1000);
        compare(r.run, "");
        compare(G.powerLabel(r.armed, "reboot", 1500), "Click again to restart");
        compare(G.powerLabel(r.armed, "poweroff", 1500), "Shut down");
        compare(G.powerClick(r.armed, "reboot", 2000).run, "reboot");
        // Too late, or a different button: arms again instead.
        compare(G.powerClick(r.armed, "reboot", 1000 + G.POWER_CONFIRM_MS + 1).run, "");
        var other = G.powerClick(r.armed, "poweroff", 2000);
        compare(other.run, "");
        compare(other.armed.action, "poweroff");
        compare(G.powerClick(null, "hibernate", 0), { armed: null, run: "" });
        compare(G.powerCommand("suspend"), ["systemctl", "suspend"]);
        compare(G.powerCommand("rm"), null);
        compare(G.powerFailure("reboot", "Access denied\nmore"), { text: "Couldn't restart: Access denied", error: true });
        compare(G.powerFailure("poweroff", ""), { text: "Couldn't shut down", error: true });
    }

    function test_main_screen() {
        compare(G.mainScreen(["DP-1", "eDP-1"], "eDP-1"), "eDP-1");
        compare(G.mainScreen(["DP-1", "eDP-1"], ""), "DP-1");
        compare(G.mainScreen([], "DP-1"), "");
    }
}
