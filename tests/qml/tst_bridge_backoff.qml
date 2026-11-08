import QtQuick
import QtTest
import "../../shell/logic/backoff.js" as Backoff

TestCase {
    name: "backoff"

    function test_sequence() {
        var want = [1000, 2000, 4000, 8000, 16000, 30000, 30000];
        for (var i = 0; i < want.length; i++)
            compare(Backoff.next(i), want[i], "attempt " + i);
        compare(Backoff.next(50), 30000);
        compare(Backoff.next(-1), 1000);
    }
}
