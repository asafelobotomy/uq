// The plugin page context (ctx.command, ctx.dbus) and the confirm dialog, against a stand-in
// for the PluginHost singleton that behaves like the Rust one: request ids at once, results and
// confirmation requests later, a confirmed command runs only after answer(id, true).
import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 600
    height: 400

    QtObject {
        id: fakeHost

        property int next: 0
        property var calls: []
        property var answers: []

        signal finished(int request, string resultJson)
        signal confirmRequested(int request, string page, string text)

        function reset() {
            next = 0;
            calls = [];
            answers = [];
        }
        function later(fn) {
            Qt.callLater(fn);
        }
        function runCommand(page, id, argsJson) {
            const req = ++next;
            calls.push(["run", page, id, argsJson]);
            const args = JSON.parse(argsJson);
            if (id === "remove")
                later(() => confirmRequested(req, page, "Remove " + args[0] + " and its leftovers?"));
            else if (id === "broken")
                later(() => finished(req, "not json"));
            else
                later(() => finished(req, JSON.stringify({ code: 0, output: { id: id, args: args } })));
            return req;
        }
        function answer(req, confirmed) {
            answers.push([req, confirmed]);
            later(() => finished(req, JSON.stringify(confirmed ? { code: 0, output: "removed" } : { error: "Cancelled", cancelled: true })));
        }
        function dbusCall(page, method, argsJson, targetJson) {
            const req = ++next;
            calls.push(["call", page, method, argsJson, targetJson]);
            later(() => finished(req, JSON.stringify({ value: [method, JSON.parse(argsJson)] })));
            return req;
        }
        function openApp(page, desktopId) {
            const req = ++next;
            calls.push(["open", page, desktopId]);
            later(() => finished(req, JSON.stringify(desktopId.endsWith(".desktop") ? { ok: true } : { error: "not an app id" })));
            return req;
        }
        function dbusGet(page, property, targetJson) {
            const req = ++next;
            calls.push(["get", page, property, targetJson]);
            later(() => finished(req, JSON.stringify({ error: "nope" })));
            return req;
        }
    }

    Component {
        id: contextComp
        PluginContext {
            pageId: "demo"
            theme: ({ accent: "#ff0000" })
        }
    }

    Component {
        id: dialogComp
        CommandConfirmDialog {}
    }

    TestCase {
        id: tc
        name: "PluginContext"
        when: windowShown

        property var results: []

        function init() {
            fakeHost.reset();
            results = [];
        }

        function make(host) {
            const c = createTemporaryObject(contextComp, root, { host: host });
            const d = createTemporaryObject(dialogComp, root);
            c.confirmNeeded.connect((req, text) => d.ask(req, text));
            d.answered.connect((req, ok) => c.answer(req, ok));
            return [c, d];
        }

        function collect(r) {
            tc.results.push(r);
        }

        function test_theme_and_shape() {
            const [c] = make(fakeHost);
            compare(c.ctx.theme.accent, "#ff0000");
            compare(typeof c.ctx.command.run, "function");
            compare(typeof c.ctx.dbus.call, "function");
            compare(typeof c.ctx.dbus.get, "function");
            compare(typeof c.ctx.openApp, "function");
        }

        function test_run_without_confirm() {
            const [c, d] = make(fakeHost);
            c.ctx.command.run("status", ["a", 2], collect);
            compare(fakeHost.calls[0], ["run", "demo", "status", "[\"a\",2]"]);
            tryCompare(tc.results, "length", 1);
            compare(tc.results[0].code, 0);
            compare(tc.results[0].output.id, "status");
            verify(!d.opened, "no dialog without confirm");
        }

        function test_confirm_runs_only_on_confirm() {
            const [c, d] = make(fakeHost);
            c.ctx.command.run("remove", ["Maps"], collect);
            tryCompare(d, "opened", true);
            compare(d.questionLabel.text, "Remove Maps and its leftovers?");
            compare(d.questionLabel.textFormat, Text.PlainText);
            compare(fakeHost.answers.length, 0, "nothing runs before an answer");
            keyClick(Qt.Key_Return);
            keyClick(Qt.Key_Enter);
            compare(fakeHost.answers.length, 0, "Enter is not Confirm");
            d.confirmButton.clicked();
            compare(fakeHost.answers, [[1, true]]);
            tryCompare(tc.results, "length", 1);
            compare(tc.results[0].output, "removed");
            tryCompare(d, "opened", false);
        }

        function test_cancel_and_escape_decline() {
            const [c, d] = make(fakeHost);
            c.ctx.command.run("remove", ["A"], collect);
            tryCompare(d, "opened", true);
            d.cancelButton.clicked();
            tryCompare(tc.results, "length", 1);
            verify(tc.results[0].cancelled);
            c.ctx.command.run("remove", ["B"], collect);
            tryCompare(d, "opened", true);
            keyClick(Qt.Key_Escape);
            tryCompare(tc.results, "length", 2);
            compare(fakeHost.answers, [[1, false], [2, false]]);
        }

        function test_a_second_question_cancels_the_first() {
            const [c, d] = make(fakeHost);
            c.ctx.command.run("remove", ["A"], collect);
            c.ctx.command.run("remove", ["B"], collect);
            tryCompare(fakeHost.answers, "length", 1);
            compare(fakeHost.answers[0], [1, false]);
            compare(d.questionLabel.text, "Remove B and its leftovers?");
            d.confirmButton.clicked();
            compare(fakeHost.answers[1], [2, true]);
        }

        function test_other_requests_are_ignored() {
            const [c] = make(fakeHost);
            c.ctx.command.run("status", [], collect);
            fakeHost.finished(999, JSON.stringify({ code: 0 }));
            tryCompare(tc.results, "length", 1);
            wait(50);
            compare(tc.results.length, 1);
        }

        function test_bad_answers_and_throwing_callbacks() {
            const [c] = make(fakeHost);
            c.ctx.command.run("broken", [], collect);
            tryCompare(tc.results, "length", 1);
            verify(tc.results[0].error.length > 0);
            ignoreWarning(/its callback failed/);
            c.ctx.command.run("status", [], () => {
                throw new Error("page bug");
            });
            c.ctx.command.run("status", [], collect);
            tryCompare(tc.results, "length", 2);
        }

        function test_dbus_requests() {
            const [c] = make(fakeHost);
            c.ctx.dbus.call("Pause", [1], collect);
            c.ctx.dbus.get("Count", collect, { path: "/o", interface: "org.x.I" });
            c.ctx.dbus.call("Resume", undefined, collect);
            compare(fakeHost.calls[0], ["call", "demo", "Pause", "[1]", ""]);
            compare(fakeHost.calls[1], ["get", "demo", "Count", "{\"path\":\"/o\",\"interface\":\"org.x.I\"}"]);
            compare(fakeHost.calls[2][3], "[]");
            tryCompare(tc.results, "length", 3);
            compare(tc.results[0].value[0], "Pause");
            compare(tc.results[1].error, "nope");
        }

        function test_open_app() {
            const [c] = make(fakeHost);
            c.ctx.openApp("org.example.App.desktop", collect);
            c.ctx.openApp("../x", collect);
            c.ctx.openApp("org.example.App.desktop");
            compare(fakeHost.calls[0], ["open", "demo", "org.example.App.desktop"]);
            tryCompare(tc.results, "length", 2);
            verify(tc.results[0].ok);
            verify(tc.results[1].error.length > 0);
        }

        function test_without_a_host_everything_fails_softly() {
            const [c] = make(null);
            c.ctx.command.run("status", [], collect);
            c.ctx.dbus.call("Pause", [], collect);
            c.ctx.dbus.get("Count", collect);
            c.ctx.openApp("x.desktop", collect);
            tryCompare(tc.results, "length", 4);
            verify(tc.results.every(r => r.error.length > 0));
        }
    }
}
