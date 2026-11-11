import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "AutostartRow"
    visible: true
    when: windowShown
    width: 800
    height: 400

    Component {
        id: comp
        AutostartRow {
            width: 800
            opensFiles: false
        }
    }

    function app(extra) {
        return Object.assign({
            id: "nm-applet.desktop",
            name: "Network",
            icon: "",
            source: "system",
            enabled: true,
            can_toggle: true,
            delay: 0,
            status: "running",
            status_text: "Running",
            reason: null,
            exit: null,
            signal: null,
            log: [],
            command_text: null,
            origin: { source: "uq", detail: null }
        }, extra || {});
    }
    function make(extra) {
        const row = createTemporaryObject(comp, tc, { app: app(extra) });
        verify(row);
        return row;
    }
    function child(row, name) {
        const c = findChild(row, name);
        verify(c, "no " + name);
        return c;
    }

    function test_name_is_plain_text() {
        const row = make({ name: "<b>x</b>" });
        const label = child(row, "appName");
        compare(label.text, "<b>x</b>");
        compare(label.textFormat, Text.PlainText);
        compare(child(row, "appStatus").textFormat, Text.PlainText);
    }

    function test_log_lines_plain() {
        const row = make({
            status: "failed",
            status_text: "Failed (exit 1)",
            exit: 1,
            log: ["<i>oops</i>", "second line"]
        });
        const toggle = child(row, "logToggle");
        verify(toggle.visible, "a failed app offers what it printed");
        toggle.toggle();
        const first = child(row, "logLine0");
        compare(first.text, "<i>oops</i>");
        compare(first.textFormat, Text.PlainText);
        compare(child(row, "logLine1").text, "second line");
        verify(!child(make({ log: ["x"] }), "logToggle").visible, "only on failure");
    }

    function test_switch_disabled_when_cannot_toggle() {
        verify(child(make(), "appSwitch").enabled);
        const row = make({ can_toggle: false, status: "skipped", status_text: "Not installed" });
        verify(!child(row, "appSwitch").enabled);
        verify(!child(row, "appDelay").enabled);
        compare(child(row, "appStatus").text, "Not installed");
        const idle = make();
        idle.idle = false;
        verify(!child(idle, "appSwitch").enabled, "nothing while the page is busy");
    }

    function test_remove_only_for_added() {
        verify(!child(make({ source: "system" }), "removeApp").visible);
        verify(!child(make({ source: "yours" }), "removeApp").visible);
        const added = make({ id: "a-3f9c", source: "added", command_text: "backup-check --quiet" });
        verify(child(added, "removeApp").visible);
        compare(child(added, "appSource").text, "Added in uQ");
        compare(child(added, "appCommand").text, "backup-check --quiet");
        compare(child(make({ source: "yours" }), "appSource").text, "Yours");
        compare(child(make(), "appSource").text, "From the system");
    }

    function test_origin_line_from_source() {
        const row = make({ origin: { source: "user_lua", detail: { line: 7, path: "/home/u/.config/hypr/user.lua" } } });
        const origin = child(row, "appOrigin");
        compare(origin.text, "Overridden in user.lua, line 7");
        compare(origin.textFormat, Text.PlainText);
        verify(origin.visible);
        // Shown once: not also in the source or status lines.
        verify(child(row, "appSource").text.indexOf("user.lua") < 0);
        verify(child(row, "appStatus").text.indexOf("user.lua") < 0);
        const plain = make();
        compare(child(plain, "appOrigin").text, "", "uQ's own value says nothing");
        const failed = make({ origin: { source: "error", detail: { message: "<b>no</b>" } } });
        compare(child(failed, "appOrigin").text, "<b>no</b>");
    }
}
