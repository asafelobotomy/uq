import QtQuick
import QtTest
import "../../shell/logic/failure.js" as F

// shell/logic/failure.js: one short plain line for a bridge call that failed.
TestCase {
    name: "failure"

    function test_failure_text() {
        compare(F.failureText("Couldn't open Firefox", "no such app"), "Couldn't open Firefox: no such app");
        compare(F.failureText("Couldn't open Firefox", ""), "Couldn't open Firefox.");
        compare(F.failureText("Couldn't open Firefox", undefined), "Couldn't open Firefox.");
        compare(F.failureText("Couldn't switch the theme", "uqd is not running"),
                "Couldn't switch the theme: uQ's settings service isn't reachable.");
        compare(F.failureText("x", "reset"), "x: uQ's settings service isn't reachable.");
        compare(F.failureText("x", "line\nbreak‮end"), "x: line break end");
        compare(F.failureText("x", { message: "boom" }), "x: boom");
        var cut = F.failureText("x", new Array(400).join("a"));
        compare(cut.length, "x: ".length + F.MAX_REASON);
        verify(cut.endsWith("…"));
    }
}
