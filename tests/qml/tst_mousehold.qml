import QtQuick
import QtTest
import "../../shell/logic/mousehold.js" as Hold

TestCase {
    name: "mousehold"

    function test_kind() {
        compare(Hold.kind("custom", "uq-mouse:begin"), "begin");
        compare(Hold.kind("custom", "uq-mouse:end"), "end");
        // Other custom events, other events with the same text, and odd input are nothing.
        compare(Hold.kind("custom", "something-else"), null);
        compare(Hold.kind("custom", "uq-mouse:begin,0"), null);
        compare(Hold.kind("openwindow", "uq-mouse:begin"), null);
        compare(Hold.kind("custom", ""), null);
        compare(Hold.kind("custom", undefined), null);
        compare(Hold.kind("custom", null), null);
        compare(Hold.kind(undefined, "uq-mouse:end"), null);
    }
}
