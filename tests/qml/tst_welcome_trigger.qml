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

    // Hello says due, the grace period passes, WelcomeStatus answers `status`.
    function run(status) {
        var r = W.onEvent(W.initial(), "Hello", hello(true, false));
        compare(r.action, "wait");
        r = W.onTimer(r.state);
        compare(r.action, "ask");
        return W.onStatus(r.state, status);
    }

    function test_due_starts_once_after_the_grace_period() {
        verify(W.GRACE_MS >= 10000, "long enough for an autostarted installer");
        var r = run({ pending: true, deferred: false });
        compare(r.action, "start");
        verify(r.state.decided);
        // A bridge restart sends Hello again: never a second Welcome from this shell.
        compare(W.onEvent(r.state, "Hello", hello(true, false)).action, "none");
        compare(W.onEvent(r.state, "UqdUp", null).action, "none");
        compare(W.onTimer(r.state).action, "none");
        compare(W.onStatus(r.state, { pending: true, deferred: false }).action, "none");
    }

    function test_installer_defers_during_the_grace_period() {
        var r = run({ pending: true, deferred: true });
        compare(r.action, "none");
        verify(r.state.decided, "the installer runs `uqctl welcome` when its wizard ends");
    }

    function test_done_never_starts() {
        var r = W.onEvent(W.initial(), "Hello", hello(false, false));
        compare(r.action, "none");
        verify(r.state.decided);
        compare(W.onEvent(r.state, "UqdUp", null).action, "none");
        // Finished during the grace period (uqctl welcome by hand): nothing more.
        compare(run({ pending: false, deferred: false }).action, "none");
    }

    function test_deferred_at_hello_is_left_to_the_installer() {
        var r = W.onEvent(W.initial(), "Hello", hello(true, true));
        compare(r.action, "none");
        verify(r.state.decided);
        compare(W.onEvent(r.state, "UqdUp", null).action, "none");
    }

    function test_uqd_late_waits_then_asks() {
        var r = W.onEvent(W.initial(), "Hello", hello(null, false));
        compare(r.action, "none");
        verify(!r.state.decided && !r.state.waiting);
        compare(W.onTimer(r.state).action, "none", "no timer runs yet");
        r = W.onEvent(r.state, "UqdUp", null);
        compare(r.action, "wait");
        // A second UqdUp while waiting doesn't start another wait.
        compare(W.onEvent(r.state, "UqdUp", null).action, "none");
        r = W.onTimer(r.state);
        compare(r.action, "ask");
        compare(W.onStatus(r.state, { pending: true, deferred: false }).action, "start");
    }

    function test_failed_status_waits_again_on_next_uqd_up() {
        var r = W.onEvent(W.initial(), "UqdUp", null);
        r = W.onStatus(W.onTimer(r.state).state, null);
        compare(r.action, "none");
        verify(!r.state.decided && !r.state.waiting);
        compare(W.onEvent(r.state, "UqdUp", null).action, "wait");
        // An answer without a boolean `pending` is unknown too.
        r = W.onStatus({ decided: false, waiting: true }, { pending: null, deferred: false });
        verify(!r.state.decided);
    }

    function test_old_bridge_hello_without_welcome_fields() {
        var r = W.onEvent(W.initial(), "Hello", { v: 1, uqd_up: true, locked: false });
        compare(r.action, "none");
        verify(!r.state.decided);
        compare(W.onEvent(W.initial(), "Hello", null).action, "none");
        compare(W.onEvent(W.initial(), "ThemeChanged", {}).action, "none");
    }
}
