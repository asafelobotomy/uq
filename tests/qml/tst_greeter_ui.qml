import QtQuick
import QtTest
import "../../greeter"
import "../../shell/logic/greeter.js" as G

// greeter/Auth.qml and greeter/Greeter.qml against a fake Greetd (the same API as
// Quickshell.Services.Greetd), offscreen.
TestCase {
    id: tc
    name: "greeter_ui"
    width: 1000; height: 700
    when: windowShown

    Component {
        id: fakeC
        QtObject {
            property var calls: []
            signal authMessage(string message, bool error, bool responseRequired, bool echoResponse)
            signal authFailure(string message)
            signal readyToLaunch()
            signal launched()
            signal error(string error)
            function createSession(user) { calls.push(["create", user]); }
            function respond(text) { calls.push(["respond", text]); }
            function cancelSession() { calls.push(["cancel"]); }
            function launch(command, env, quit) { calls.push(["launch", command, env, quit]); }
        }
    }
    Component { id: authC; Auth {} }
    Component { id: greeterC; Greeter {} }
    SignalSpy { id: launchingSpy; signalName: "launching" }
    SignalSpy { id: launchedSpy; signalName: "launched" }
    SignalSpy { id: powerSpy; signalName: "powerRequested" }

    property var sessions: G.sessionList(G.parseSessionDump(
        "\u001esway.desktop\u001f[Desktop Entry]\nName=Sway\nExec=sway\n"
        + "\u001euq.desktop\u001f[Desktop Entry]\nName=uQ\nExec=/usr/bin/uq-session\n"))
    property var users: G.visibleUsers([{ name: "alice", realName: "Alice", icon: "" },
                                        { name: "bob", realName: "", icon: "" }])

    function make(extra) {
        var fake = createTemporaryObject(fakeC, tc);
        var auth = createTemporaryObject(authC, tc, { greetd: fake });
        var props = { auth: auth, users: users, sessions: sessions, width: 1000, height: 700 };
        for (var k in (extra || {}))
            if (k !== "preferred")
                props[k] = extra[k];
        var g = createTemporaryObject(greeterC, tc, props);
        g.start(extra && extra.preferred ? extra.preferred : "");
        return { fake: fake, auth: auth, g: g };
    }

    function find(item, name) {
        if (!item)
            return null;
        if (item.objectName === name)
            return item;
        var kids = item.children || [];
        for (var i = 0; i < kids.length; i++) {
            var f = find(kids[i], name);
            if (f)
                return f;
        }
        return null;
    }

    function enter(field, text) {
        field.text = text;
        field.accepted();
    }

    // greeter.toml's reduce_motion reaches the user list: no coasting, no animated scroll.
    function test_reduce_motion_reaches_the_user_list() {
        var t = make({});
        var list = find(t.g, "userList");
        verify(list);
        compare(list.reduceMotion, false);
        verify(list.flickDeceleration < 100000);
        var r = make({ reduceMotion: true });
        list = find(r.g, "userList");
        compare(list.reduceMotion, true);
        compare(list.flickDeceleration, 100000);
        compare(list.highlightMoveDuration, 0);
    }

    function test_selects_last_user_and_their_session() {
        var t = make({ stateData: G.parseState('last_user = "bob"\n[sessions]\nbob = "sway.desktop"\n') });
        compare(t.g.selectedUser, "bob");
        compare(t.g.session.id, "sway.desktop");
        t.g.selectUser("alice", true);
        compare(t.g.session.id, "uq.desktop");  // uQ first and preselected
        compare(t.auth.session.id, "uq.desktop");
    }

    function test_password_login_launches_the_session() {
        var t = make();
        launchingSpy.target = t.auth;
        launchingSpy.clear();
        compare(t.g.selectedUser, "alice");
        var pw = find(t.g, "passwordField");
        enter(pw, "hunter2");
        compare(pw.text, "");  // cleared after the attempt
        compare(t.fake.calls, [["create", "alice"]]);
        t.fake.authMessage("Password: ", false, true, false);
        compare(t.fake.calls[1], ["respond", "hunter2"]);
        compare(t.auth.mode, "busy");
        t.fake.readyToLaunch();
        compare(launchingSpy.count, 1);
        compare(launchingSpy.signalArguments[0][0], "alice");
        compare(launchingSpy.signalArguments[0][1], "uq.desktop");
        compare(t.fake.calls[2], ["launch", ["/usr/bin/uq-session"], ["XDG_SESSION_TYPE=wayland"], false]);
        launchedSpy.target = t.auth;
        launchedSpy.clear();
        t.fake.launched();
        compare(launchedSpy.count, 1);
    }

    function test_wrong_password_and_faillock() {
        var t = make();
        enter(find(t.g, "passwordField"), "nope");
        t.fake.authMessage("The account is locked due to 3 failed logins.", true, false, false);
        t.fake.authMessage("(10 minutes left to unlock)", false, false, false);
        t.fake.authFailure("pam_authenticate: AUTH_ERR");
        compare(t.auth.mode, "password");
        compare(find(t.g, "message").text,
                "The account is locked due to 3 failed logins. (10 minutes left to unlock)");
        compare(t.auth.message.error, true);
        t.fake.authFailure("again");  // a stray failure keeps the greeter usable
        compare(t.auth.mode, "password");
    }

    function test_echo_on_prompt_shows_a_visible_field() {
        var t = make();
        enter(find(t.g, "passwordField"), "pw");
        t.fake.authMessage("One-time code:", false, true, true);
        var vis = find(t.g, "visibleField");
        compare(t.auth.mode, "visible");
        compare(find(t.g, "promptLabel").text, "One-time code");
        compare(find(t.g, "promptLabel").textFormat, Text.PlainText);
        compare(find(t.g, "passwordField").placeholder, "Password");
        enter(vis, "424242");
        compare(t.fake.calls[1], ["respond", "424242"]);
        compare(vis.text, "");
    }

    function test_expired_password_form() {
        var t = make();
        enter(find(t.g, "passwordField"), "old");
        t.fake.authMessage("Password: ", false, true, false);
        t.fake.authMessage("You are required to change your password immediately.", true, false, false);
        t.fake.authMessage("New password: ", false, true, false);
        compare(t.auth.mode, "new");
        var nf = find(t.g, "newField"), cf = find(t.g, "confirmField");
        nf.text = "Correct1horse";
        enter(cf, "Correct1hors");
        compare(t.fake.calls.length, 2);  // the mismatch never reached greetd
        compare(find(t.g, "message").text, "The new passwords don't match");
        compare(nf.text, "");
        compare(cf.text, "");
        nf.text = "Correct1horse";
        enter(cf, "Correct1horse");
        compare(t.fake.calls[2], ["respond", "Correct1horse"]);
        t.fake.authMessage("Retype new password: ", false, true, false);
        compare(t.fake.calls[3], ["respond", "Correct1horse"]);
    }

    function test_escape_and_user_switch_cancel() {
        var t = make();
        enter(find(t.g, "passwordField"), "");
        t.fake.authMessage("Password:", false, true, false);
        compare(t.auth.mode, "secret");
        t.g.cancel();
        compare(t.fake.calls[1], ["cancel"]);
        compare(t.auth.mode, "password");
        enter(find(t.g, "passwordField"), "pw");
        find(t.g, "userList").chosen("bob");
        compare(t.fake.calls[3], ["cancel"]);
        compare(t.g.selectedUser, "bob");
        compare(t.auth.user, "bob");
    }

    function test_hidden_user_list_uses_a_name_field() {
        var t = make({ hideUsers: true, stateData: G.parseState('last_user = "alice"\n') });
        var nf = find(t.g, "nameField");
        compare(t.g.listShown, false);
        compare(nf.text, "");  // hidden list: no name shown
        nf.text = "carol";
        enter(find(t.g, "passwordField"), "pw");
        compare(t.fake.calls[0], ["create", "carol"]);
    }

    function test_no_accountsservice_prefills_last_user() {
        var t = make({ users: [], stateData: G.parseState('last_user = "alice"\n') });
        compare(find(t.g, "nameField").text, "alice");
    }

    function test_session_ended_early_keeps_user() {
        var t = make({ preferred: "bob" });
        compare(t.g.selectedUser, "bob");
        t.auth.notify({ text: G.ENDED_EARLY_TEXT, error: true });
        compare(find(t.g, "message").text, "The session ended straight away");
    }

    function test_power_needs_confirmation() {
        var t = make();
        powerSpy.target = t.g;
        powerSpy.clear();
        var reboot = find(t.g, "power-reboot");
        compare(reboot.label, "Restart");
        reboot.activated();
        compare(powerSpy.count, 0);
        compare(reboot.label, "Click again to restart");
        reboot.activated();
        compare(powerSpy.count, 1);
        compare(powerSpy.signalArguments[0][0], "reboot");
        compare(reboot.label, "Restart");
    }

    function test_session_picker_cycles() {
        var t = make();
        var p = find(t.g, "sessionPicker");
        compare(p.current.name, "uQ");
        p.step(1);
        compare(t.g.session.name, "Sway");
        p.step(1);
        compare(t.g.session.name, "uQ");
    }

    function test_no_session_to_launch() {
        var t = make({ sessions: [] });
        enter(find(t.g, "passwordField"), "pw");
        t.fake.authMessage("Password:", false, true, false);
        t.fake.readyToLaunch();
        compare(t.fake.calls[2], ["cancel"]);
        compare(t.auth.message.text, "There is no session to start");
        compare(t.auth.mode, "password");
    }
}
