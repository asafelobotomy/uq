import QtQuick
import QtTest
import "../../shell/logic/minimise.js" as Min

TestCase {
    name: "minimise"

    function test_roundtrip() {
        compare(Min.tagFor(3), "uq_ws_3");
        compare(Min.wsFromTags([Min.tagFor(7)]), 7);
    }
    function test_found_among_others() { compare(Min.wsFromTags(["foo", "uq_ws_3"]), 3); }
    function test_dynamic_tag_suffix() { compare(Min.wsFromTags(["uq_ws_12*"]), 12); }
    function test_no_tag() {
        compare(Min.wsFromTags(["foo"]), null);
        compare(Min.wsFromTags([]), null);
        compare(Min.wsFromTags(null), null);
        compare(Min.wsFromTags(undefined), null);
    }
    function test_malformed() {
        compare(Min.wsFromTags(["uq_ws_x"]), null);
        compare(Min.wsFromTags(["uq_ws_"]), null);
        compare(Min.wsFromTags(["uq_ws_-1"]), null);
        compare(Min.wsFromTags(["uq_ws_3x"]), null);
    }
}
