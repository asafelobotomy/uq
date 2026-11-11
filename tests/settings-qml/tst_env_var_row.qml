import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "EnvVarRow"
    visible: true
    when: windowShown
    width: 800
    height: 300

    Component {
        id: comp
        EnvVarRow {
            width: 800
            opensFiles: false
        }
    }

    Component {
        id: spyComp
        SignalSpy {}
    }

    function make(props) {
        const row = createTemporaryObject(comp, tc, Object.assign({ name: "GDK_BACKEND", value: "wayland" }, props || {}));
        verify(row);
        return row;
    }
    function child(row, name) {
        const c = findChild(row, name);
        verify(c, "no " + name);
        return c;
    }

    function test_warning_shown_for_warned() {
        verify(!child(make(), "varWarning").visible);
        const row = make({ name: "LD_PRELOAD", value: "/x.so", warn: true });
        const warning = child(row, "varWarning");
        verify(warning.visible);
        compare(warning.text, "This can stop apps from starting. Remove it if apps break.");
        compare(warning.textFormat, Text.PlainText);
    }

    function test_value_plain_text() {
        const row = make({ value: "<b>x</b>" });
        compare(child(row, "varValue").text, "<b>x</b>");
        compare(child(row, "varName").text, "GDK_BACKEND");
        verify(!row.dirty, "the boxes hold the saved value");
        verify(!child(row, "saveVar").visible);
        // Typing offers Save; Save sends the boxes.
        const spy = createTemporaryObject(spyComp, tc, { target: row, signalName: "saveRequested" });
        const value = child(row, "varValue");
        value.text = "x11";
        value.textEdited();
        verify(row.dirty);
        verify(child(row, "saveVar").visible);
        child(row, "saveVar").clicked();
        compare(spy.count, 1);
        compare(spy.signalArguments[0][0], "GDK_BACKEND");
        compare(spy.signalArguments[0][1], "x11");
        // The new state puts the saved value back.
        row.value = "x11";
        verify(!row.dirty);
    }

    function test_replaced_line() {
        verify(!child(make(), "varReplaced").visible);
        const row = make({ name: "QT_QPA_PLATFORM", value: "xcb", replacedBy: "Qt apps prefer Wayland" });
        const line = child(row, "varReplaced");
        verify(line.visible);
        compare(line.text, "Not used: the “Qt apps prefer Wayland” switch on Apps and scaling sets this name.");
        compare(line.textFormat, Text.PlainText);
        const lua = make({ origin: { source: "user_lua", detail: { line: 4, path: "/home/u/.config/hypr/user.lua" } } });
        compare(child(lua, "varOrigin").text, "Overridden in user.lua, line 4");
    }
}
