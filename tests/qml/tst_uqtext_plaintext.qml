import QtQuick
import QtTest
import "../../shell/core"

TestCase {
    name: "uqtext"
    width: 400; height: 100
    when: windowShown

    Component { id: c; UqText {} }

    function test_plain_text() {
        var a = createTemporaryObject(c, this, { text: "<b>x</b><img src='http://127.0.0.1:1/'>" });
        var b = createTemporaryObject(c, this, { text: "x" });
        compare(a.textFormat, Text.PlainText);
        compare(a.elide, Text.ElideRight);
        verify(a.implicitWidth > b.implicitWidth);
    }
}
