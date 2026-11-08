import QtQuick
import QtTest
import "../../shell/logic/work.js" as Work

TestCase {
    name: "work"
    function test_minimiseFrom() {
        compare(Work.minimiseFrom({ id: 3, name: "3" }), 3);
        compare(Work.minimiseFrom({ id: -98, name: "special:minimized" }), null);
        compare(Work.minimiseFrom({ id: 0, name: "x" }), null);
        compare(Work.minimiseFrom(null), null);
        compare(Work.minimiseFrom({ id: "3" }), null);
        compare(Work.minimiseFrom({ id: 2.5 }), null);
    }
    function test_wsChoices() {
        var c = Work.wsChoices();
        compare(c.length, 10);
        compare(c[0], 1);
        compare(c[9], 10);
    }
    function test_normAddr() {
        compare(Work.normAddr("55d0aa"), "0x55d0aa");
        compare(Work.normAddr("0x55d0aa"), "0x55d0aa");
        compare(Work.normAddr("0xZZ"), null);
        compare(Work.normAddr(""), null);
        compare(Work.normAddr("0x12345678901234567"), null);
    }
}
