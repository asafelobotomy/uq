import QtQuick
import QtTest
import "../../shell/logic/welcome.js" as W

// When the shell starts Welcome (shell/logic/welcome.js, Phase 5 spec §5.1–5.2).
TestCase {
    name: "welcome_trigger"

    function hello(pending, deferred) {
        return { v: 1, uqd_up: pending !== null, locked: false,
                 welcome_pending: pending, welcome_deferred: deferred };
    }

    function test_due_and_not_deferred_starts_once() {
        var r = W.onEvent(W.initial(), "Hello", hello(true, false));
        compare(r.action, "start");
        verify(r.state.decided);
        // A bridge restart sends Hello again: never a second Welcome from this shell.
        r = W.onEvent(r.state, "Hello", hello(true, false));
        compare(r.action, "none");
        r = W.onEvent(r.state, "UqdUp", null);
        compare(r.action, "none");
    }

    function test_done_never_starts() {
        var r = W.onEvent(W.initial(), "Hello", hello(false, false));
        compare(r.action, "none");
        verify(r.state.decided);
        compare(W.onEvent(r.state, "UqdUp", null).action, "none");
    }

    function test_deferred_is_left_to_bindle() {
        var r = W.onEvent(W.initial(), "Hello", hello(true, true));
        compare(r.action, "none");
        verify(r.state.decided, "bindle runs `uqctl welcome` when its wizard ends");
        compare(W.onEvent(r.state, "UqdUp", null).action, "none");
    }

    function test_uqd_late_asks_once_then_decides() {
        var r = W.onEvent(W.initial(), "Hello", hello(null, false));
        compare(r.action, "none");
        verify(!r.state.decided);
        r = W.onEvent(r.state, "UqdUp", null);
        compare(r.action, "ask");
        // A second UqdUp while the first question is out doesn't ask again.
        compare(W.onEvent(r.state, "UqdUp", null).action, "none");
        var s = W.onStatus(r.state, { pending: true, deferred: false });
        compare(s.action, "start");
        compare(W.onStatus(s.state, { pending: true, deferred: false }).action, "none");
    }

    function test_late_status_deferred_or_done() {
        var asked = W.onEvent(W.onEvent(W.initial(), "Hello", hello(null, false)).state, "UqdUp", null).state;
        compare(W.onStatus(asked, { pending: true, deferred: true }).action, "none");
        compare(W.onStatus(asked, { pending: false, deferred: false }).action, "none");
    }

    function test_failed_status_asks_again_on_next_uqd_up() {
        var asked = W.onEvent(W.onEvent(W.initial(), "Hello", hello(null, false)).state, "UqdUp", null).state;
        var r = W.onStatus(asked, null);
        compare(r.action, "none");
        verify(!r.state.decided && !r.state.asked);
        compare(W.onEvent(r.state, "UqdUp", null).action, "ask");
        // An answer without a boolean `pending` is unknown too.
        r = W.onStatus(asked, { pending: null, deferred: false });
        verify(!r.state.decided);
    }

    function test_old_bridge_hello_without_welcome_fields() {
        var r = W.onEvent(W.initial(), "Hello", { v: 1, uqd_up: true, locked: false });
        compare(r.action, "none");
        verify(!r.state.decided);
        r = W.onEvent(W.initial(), "Hello", null);
        compare(r.action, "none");
        compare(W.onEvent(W.initial(), "ThemeChanged", {}).action, "none");
    }
}
