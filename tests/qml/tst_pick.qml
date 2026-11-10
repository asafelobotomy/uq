import QtQuick
import QtTest
import "../../shell/logic/pick.js" as P

TestCase {
    name: "pick"

    readonly property string tokA: "0123456789abcdef0123456789abcdef"
    readonly property string tokB: "fedcba9876543210fedcba9876543210"

    function test_bad_token_ignored() {
        var s = P.idle();
        var bad = ["", "xyz", tokA.toUpperCase(), tokA + "0", tokA.slice(1), null, undefined, 5, {}];
        for (var i = 0; i < bad.length; i++) {
            var r = P.start(s, bad[i], false);
            compare(r.reply, null);
            verify(!P.isActive(r.state));
        }
        // A bad token never disturbs a pick that is already showing.
        var shown = P.start(s, tokA, false).state;
        var r2 = P.start(shown, "nope", false);
        compare(r2.reply, null);
        compare(r2.state.token, tokA);
        verify(P.validToken(tokA));
    }

    function test_locked_cancels() {
        var r = P.start(P.idle(), tokA, true);
        compare(r.reply, ["cancel"]);
        compare(r.token, tokA);
        verify(!P.isActive(r.state));
        // Nothing further to send for it.
        compare(P.click(r.state).reply, null);
        compare(P.timeout(r.state).reply, null);
    }

    function test_click_replies_once() {
        var r = P.start(P.idle(), tokA, false);
        compare(r.reply, ["shown"]);
        verify(P.isActive(r.state));
        var c = P.click(r.state);
        compare(c.reply, ["click"]);
        compare(c.token, tokA);
        verify(!P.isActive(c.state));
        compare(P.click(c.state).reply, null);
        compare(P.escape(c.state).reply, null);
        compare(P.timeout(c.state).reply, null);
    }

    function test_escape_and_timeout() {
        var s = P.start(P.idle(), tokA, false).state;
        var e = P.escape(s);
        compare(e.reply, ["cancel"]);
        verify(!P.isActive(e.state));
        compare(P.escape(e.state).reply, null);
        var t = P.timeout(s);
        compare(t.reply, ["cancel"]);
        compare(t.token, tokA);
        compare(P.timeout(t.state).reply, null);
        compare(P.PICK_TIMEOUT_MS, 20000);
    }

    function test_new_token_replaces_old() {
        var a = P.start(P.idle(), tokA, false).state;
        var b = P.start(a, tokB, false);
        compare(b.reply, ["shown"]);
        compare(b.state.token, tokB);
        var c = P.click(b.state);
        compare(c.token, tokB);
        compare(c.reply, ["click"]);
    }
}
