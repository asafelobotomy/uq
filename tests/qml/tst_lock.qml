import QtQuick
import QtTest
import "../../shell/logic/lock.js" as L

TestCase {
    name: "lock"

    function test_fprint_listed() {
        var enrolled = "found 1 devices\nDevice at /net/reactivated/Fprint/Device/0\nUsing device /net/reactivated/Fprint/Device/0\nFingerprints for user solon on Goodix MOC Fingerprint Sensor (press):\n - #0: right-index-finger\n - #1: left-index-finger\n";
        var none = "found 1 devices\nDevice at /net/reactivated/Fprint/Device/0\nUsing device /net/reactivated/Fprint/Device/0\nUser solon has no fingers enrolled for Goodix MOC Fingerprint Sensor.";
        compare(L.fprintListed(enrolled, 0), true);
        compare(L.fprintListed(enrolled, 1), false);
        compare(L.fprintListed(none, 0), false);
        compare(L.fprintListed("No devices available", 0), false);
        compare(L.fprintListed("No devices available", 1), false);
        compare(L.fprintListed("Failed to connect to the daemon", 1), false);
        compare(L.fprintListed(null, 0), false);
        compare(L.fprintListed("", 0), false);
    }

    function test_avatar_order() {
        compare(L.avatarSource("/var/lib/icons/s.png", true, "/home/solon").path, "file:///var/lib/icons/s.png");
        compare(L.avatarSource("/a b/#.png", false, "/h").path, "file:///a%20b/%23.png");
        compare(L.avatarSource("", true, "/home/solon").path, "file:///home/solon/.face");
        compare(L.avatarSource("", false, "/home/solon", "solon").initial, "S");
        compare(L.avatarSource("", false, "/home/solon").initial, "?");
        compare(L.avatarSource(null, false, "/h").path, undefined);
    }

    function test_initial_of() {
        compare(L.initialOf("solon"), "S");
        compare(L.initialOf("Solon"), "S");
        compare(L.initialOf("😀x"), "😀");
        compare(L.initialOf(""), "?");
        compare(L.initialOf(null), "?");
    }

    function test_fprint_restart_limit() {
        compare(L.FPRINT_RETRY_MS, 10000);
        compare(L.fprintCanRestart(0, 9999), false);
        compare(L.fprintCanRestart(0, 10000), true);
        compare(L.fprintCanRestart(5000, 14999), false);
        compare(L.fprintCanRestart(5000, 15000), true);
    }

    function test_password_reply() {
        compare(L.passwordReply(true, false, "hunter2"), "hunter2");
        compare(L.passwordReply(true, true, "hunter2"), null);
        compare(L.passwordReply(false, false, "hunter2"), null);
        compare(L.passwordReply(false, true, "x"), null);
    }

    function test_fprint_abort() {
        compare(L.fprintAbortOnPrompt(true), true);
        compare(L.fprintAbortOnPrompt(false), false);
    }

    function test_message_view() {
        compare(L.messageView("", false).text, "");
        compare(L.messageView(null, true).text, "");
        compare(L.messageView(null, true).error, false);
        var v = L.messageView("Authentication failed", true);
        compare(v.text, "Authentication failed");
        compare(v.error, true);
        compare(L.messageView("hi\nthere", false).text, "hi there");
        compare(L.messageView(new Array(300).join("a"), false).text.length, 200);
    }

    function test_sibling_shell_dir() {
        compare(L.siblingShellDir("/usr/share/uq/lock"), "/usr/share/uq/shell");
        compare(L.siblingShellDir("/usr/share/uq/lock/"), "/usr/share/uq/shell");
        compare(L.siblingShellDir("/home/u/Projects/uq/lock//"), "/home/u/Projects/uq/shell");
        compare(L.siblingShellDir("/lock"), "/shell");
    }

    function test_recovery_decision() {
        compare(L.recoveryDecision(true, false, false), "start");
        compare(L.recoveryDecision(true, true, false), "none");
        compare(L.recoveryDecision(true, false, true), "none");
        compare(L.recoveryDecision(true, true, true), "none");
        compare(L.recoveryDecision(false, false, false), "none");
        compare(L.recoveryDecision(false, true, false), "none");
        compare(L.recoveryDecision(false, false, true), "none");
    }
}
