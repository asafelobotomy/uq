import QtQuick
import QtTest
import "../../shell/core"
import "../../shell/widgets"
import "../../shell/quick"
import "../../shell/launcher"
import "../../shell/logic/widgets.js" as W

// Widgets from other apps (Phase 4 spec §2.2, §7): the slot host with a good widget, one that
// fails to load, one that throws later, overflow into "…" and the empty Apps group; plus the
// app-action helpers (§2.4.2).
TestCase {
    id: tc
    name: "widgets"
    when: windowShown
    width: 400
    height: 400

    // Stands in for the bridge: the `event` signal and `call`, answered on the next turn.
    QtObject {
        id: fake
        property var widgets: []
        property var appActions: []
        property var calls: []
        signal event(string name, var data)
        function call(name, args) {
            var c = calls.slice();
            c.push({ call: name, args: args });
            calls = c;
            var h = Bridge.makeCall();
            var answer = name === "WidgetRun" ? { code: 0, output: "done" }
                : (name === "WidgetGet" ? { Count: 1 } : null);
            Qt.callLater(function () { h._resolve(answer); });
            return h;
        }
    }

    Component {
        id: slotComp
        Slot {}
    }
    Component {
        id: hostComp
        WidgetHost {}
    }
    Component {
        id: appsComp
        AppsGroup {}
    }
    Component {
        id: menuComp
        Item {
            width: 300
            height: 300
            property alias menu: m
            AppActionMenu {
                id: m
            }
        }
    }

    function entry(id, fixture, slot, order) {
        return { id: id, name: "W " + id, slot: slot || "bar", order: order === undefined ? 50 : order,
                 qml_url: Qt.resolvedUrl("fixtures/widgets/" + fixture + "/Widget.qml").toString(),
                 visible: true };
    }

    function init() {
        fake.widgets = [];
        fake.appActions = [];
        fake.calls = [];
        Widgets.revs = ({});
        Widgets.bridge = fake;
        Widgets.failures = W.failures();
        Widgets.log = [];
        Bridge.locked = false;
    }

    function cleanup() {
        Widgets.bridge = Bridge;
    }

    function makeSlot(id, fixture) {
        fake.widgets = [entry(id, fixture)];
        return createTemporaryObject(slotComp, tc, { widget: Widgets.list[0], bridge: fake });
    }

    // --- logic ---

    function test_widgets_list_is_cleaned_and_ordered() {
        var l = W.widgets([
            { id: "b", name: "B", slot: "bar", order: 10, qml_url: "file:///w/b/W.qml", visible: true },
            { id: "a", name: "A", slot: "quick", order: 10, qml_url: "file:///w/a/W.qml", visible: false },
            { id: "c", name: "C", slot: "bar", order: 5, qml_url: "file:///w/c/W.qml", visible: true },
            { id: "Bad", name: "x", slot: "bar", qml_url: "file:///x.qml", visible: true },
            { id: "web", name: "x", slot: "bar", qml_url: "https://evil/x.qml", visible: true },
            { id: "s", name: "x", slot: "side", qml_url: "file:///x.qml", visible: true },
            { id: "b", name: "dup", slot: "bar", qml_url: "file:///x.qml", visible: true },
            null, 3
        ]);
        compare(l.map(function (w) { return w.id; }), ["c", "a", "b"]);
        compare(l[0].qmlUrl, "file:///w/c/W.qml");
        compare(W.inSlot(l, "bar").length, 2);
        compare(W.inSlot(l, "quick").length, 0, "hidden widgets don't show");
        compare(W.widgets(null), []);
        compare(W.widgets({}), []);
    }

    // Qt keeps compiled QML by URL: a widget whose files changed must load under a new spelling.
    function test_changed_files_get_a_new_url_spelling() {
        compare(W.respell("file:///w/a/W.qml", 0), "file:///w/a/W.qml");
        compare(W.respell("file:///w/a/W.qml", 2), "file:///w/a/././W.qml");
        var r = W.nextRevs({}, [{ id: "a", rev: "1" }]);
        compare(r.a, { rev: "1", gen: 0 });
        r = W.nextRevs(r, [{ id: "a", rev: "1" }]);
        compare(r.a.gen, 0, "same files, same URL");
        r = W.nextRevs(r, [{ id: "a", rev: "2" }]);
        compare(r.a.gen, 1);
        r = W.nextRevs(r, []);
        compare(r.a.gen, 1, "a widget that goes keeps its count");
        r = W.nextRevs(r, [{ id: "a", rev: "1" }]);
        compare(r.a.gen, 2, "going back to old files still gets a new spelling");
        var l = W.withRevs([{ id: "a", qmlUrl: "file:///w/a/W.qml" }, { id: "b", qmlUrl: "file:///w/b/W.qml" }],
                           { a: { rev: "x", gen: 1 } });
        compare(l[0].qmlUrl, "file:///w/a/./W.qml");
        compare(l[1].qmlUrl, "file:///w/b/W.qml");
    }

    function test_bar_layout_overflows_after_four() {
        var l = [];
        for (var i = 0; i < 6; i++)
            l.push({ id: "w" + i, slot: "bar", visible: true });
        l.push({ id: "q", slot: "quick", visible: true });
        var b = W.barLayout(l);
        compare(b.shown.length, 4);
        compare(b.overflow.map(function (w) { return w.id; }), ["w4", "w5"]);
        compare(W.barLayout(l.slice(0, 4)).overflow, []);
    }

    function test_failures_logged_once_retried_after_a_minute() {
        var reg = W.failures();
        verify(W.mayLoad(reg, "x", 1000));
        verify(W.noteFailure(reg, "x", 1000), "first failure is logged");
        verify(!W.mayLoad(reg, "x", 1000 + 59999));
        compare(W.retryIn(reg, "x", 1000 + 59000), 1000);
        verify(W.mayLoad(reg, "x", 1000 + 60000));
        verify(!W.noteFailure(reg, "x", 70000), "a second failure is not logged again");
        verify(W.mayLoad(reg, "y", 70000), "per widget");
    }

    // Lists that crossed a `var` signal parameter arrive as sequences Array.isArray() rejects.
    signal relay(var v)
    property var relayed
    function test_to_list_accepts_relayed_sequences() {
        compare(W.toList(["a", 1]), ["a", 1]);
        compare(W.toList(null), []);
        compare(W.toList("ab"), []);
        compare(W.toList({ length: "x" }), []);
        relayed = undefined;
        var got = function (v) { relayed = v; };
        relay.connect(got);
        relay({ args: ["x", true] });
        relay.disconnect(got);
        compare(W.toList(relayed.args), ["x", true]);
    }

    function test_props_merge_makes_new_objects() {
        var a = { A: 1 };
        var b = W.mergeProps(a, { B: 2 });
        compare(b, { A: 1, B: 2 });
        verify(a !== b);
        compare(W.mergeProps(null, [1]), {});
    }

    function test_app_actions_match_rows() {
        var acts = W.appActions([
            { id: "rm", name: "Uninstall…", icon: "edit-delete", match: "*", terminal: true },
            { id: "upd", name: "Check for update", icon: null, match: "appofholding-*.desktop" },
            { id: "BAD", name: "x", match: "*" },
            { id: "noname", match: "*" }
        ]);
        compare(acts.length, 2);
        compare(W.actionsFor(acts, "firefox").map(function (a) { return a.id; }), ["rm"]);
        compare(W.actionsFor(acts, "appofholding-krita.desktop").map(function (a) { return a.id; }),
                ["upd", "rm"], "sorted by name");
        compare(W.actionsFor(acts, "-rf"), [], "never an option-like id");
        compare(W.actionsFor(acts, "a b"), []);
        compare(W.actionsFor(acts, ""), []);
        compare(W.actionCall(acts[0], "firefox"),
                { call: "Launch", args: [{ app_action: "rm", desktop_id: "firefox.desktop" }] });
        compare(W.actionCall(acts[0], "../x"), null);
        verify(W.globMatch("a*b?c", "aXXbYc"));
        verify(!W.globMatch("a*b?c", "aXXbc"));
        verify(W.globMatch("*", ""));
    }

    function test_app_action_runs_through_the_bridge() {
        fake.appActions = [{ id: "rm", name: "Uninstall…", match: "*", terminal: true }];
        var a = Widgets.actionsFor("org.gnome.Maps");
        compare(a.length, 1);
        verify(Widgets.runAction(a[0], "org.gnome.Maps"));
        compare(fake.calls[0], { call: "Launch",
                                 args: [{ app_action: "rm", desktop_id: "org.gnome.Maps.desktop" }] });
        Bridge.locked = true;
        verify(!Widgets.runAction(a[0], "org.gnome.Maps"), "nothing runs while locked");
    }

    // The right-click menu: only for rows with actions; choosing one asks the bridge and closes.
    function test_app_action_menu() {
        fake.appActions = [{ id: "upd", name: "Check for update", match: "appofholding-*" }];
        var holder = createTemporaryObject(menuComp, tc);
        var m = holder.menu;
        var done = createTemporaryObject(spyComp, tc, { target: m, signalName: "done" });
        verify(!m.openAt(holder, 10, 10, "firefox"), "no actions: no menu");
        verify(!m.open);
        verify(m.openAt(holder, 290, 290, "appofholding-krita"));
        verify(m.open);
        compare(m.actions.length, 1);
        m.choose(m.actions[0]);
        verify(!m.open);
        compare(done.count, 1);
        compare(fake.calls[0].args[0], { app_action: "upd", desktop_id: "appofholding-krita.desktop" });
    }

    Component {
        id: spyComp
        SignalSpy {}
    }

    // --- slots ---

    function test_good_widget_loads_and_gets_ctx() {
        var s = makeSlot("good", "good");
        tryVerify(function () { return s.loaded; });
        verify(s.shown);
        compare(s.width, 26);
        compare(s.height, 26);
        var it = s.item;
        verify(it.ctx.Theme === Theme);
        verify(it.ctx.UqText !== undefined && it.ctx.UqInput !== undefined);
        compare(it.ctx.slot, "bar");
        verify(it.ctx.bridge === undefined, "no bridge handle");

        it.run();
        tryVerify(function () { return it.lastRun !== null; });
        compare(it.lastRun, { code: 0, output: "done" });
        compare(fake.calls[0], { call: "WidgetRun", args: ["good", "status", ["a"]] });

        fake.event("WidgetProps", { widget: "good", props: { Count: 3 } });
        compare(it.lastProps, { Count: 3 });
        compare(s.ctx.dbus.props.Count, 3);
        fake.event("WidgetSignal", { widget: "other", member: "Finished", args: [] });
        compare(it.lastSignal, null, "another widget's signal");
        fake.event("WidgetSignal", { widget: "good", member: "Finished", args: ["x", true] });
        compare(it.lastSignal, ["Finished", ["x", true]]);
        compare(Widgets.log, []);

        var calls = fake.calls.length;
        s.ctx.dbus.get(function () {});
        compare(fake.calls[calls], { call: "WidgetGet", args: ["good"] });
        tryVerify(function () { return s.ctx.dbus.props.Count === 1; });
    }

    function test_widget_reloads_when_its_files_change() {
        var e = entry("good", "good");
        e.rev = "a";
        fake.widgets = [e];
        var s = createTemporaryObject(slotComp, tc, { widget: Widgets.list[0], bridge: fake });
        tryVerify(function () { return s.loaded; });
        var first = s.loadedUrl;
        var e2 = entry("good", "good");
        e2.rev = "b";
        fake.widgets = [e2];
        verify(Widgets.list[0].qmlUrl !== first, "a new spelling of the same file");
        s.widget = Widgets.list[0];
        tryVerify(function () { return s.loaded && s.loadedUrl === Widgets.list[0].qmlUrl; });
        compare(Widgets.log, []);
    }

    function test_widget_that_fails_to_load_is_unloaded_and_logged_once() {
        ignoreWarning(/.*/);
        var s = makeSlot("broken", "broken");
        tryVerify(function () { return Widgets.log.length === 1; });
        verify(!s.loaded);
        verify(!s.shown, "takes no room");
        verify(Widgets.log[0].indexOf("broken") === 0, Widgets.log[0]);
        // Not retried within the minute, and not logged again by a second slot.
        s.load();
        verify(!s.loaded);
        var s2 = createTemporaryObject(slotComp, tc, { widget: Widgets.list[0], bridge: fake });
        wait(50);
        verify(!s2.loaded);
        compare(Widgets.log.length, 1);
    }

    function test_widget_that_throws_later_is_unloaded_then_retried() {
        ignoreWarning(/.*/);
        var s = makeSlot("thrower", "thrower");
        tryVerify(function () { return s.loaded; });
        s.item.poke();
        tryVerify(function () { return !s.loaded; });
        compare(Widgets.log.length, 1);
        verify(Widgets.log[0].indexOf("boom") >= 0, Widgets.log[0]);
        s.load();
        verify(!s.loaded, "not before a minute has passed");
        // A minute later it may load again.
        var reg = Widgets.failures;
        reg.thrower.at -= W.RETRY_MS;
        Widgets.failures = reg;
        s.load();
        tryVerify(function () { return s.loaded; });
    }

    function test_bar_overflow_goes_under_more() {
        var list = [];
        for (var i = 0; i < 6; i++)
            list.push(entry("w" + i, "good", "bar", i));
        fake.widgets = list;
        var h = createTemporaryObject(hostComp, tc, { kind: "bar", bridge: fake });
        tryCompare(h, "loadedCount", 4);
        compare(h.overflow.map(function (w) { return w.id; }), ["w4", "w5"]);
        verify(findChild(h, "more") !== null);
        verify(h.moreShown);
        // One goes away: everything fits, "…" goes, the others aren't reloaded.
        var first = h.slotAt(0);
        fake.widgets = list.slice(0, 4);
        tryCompare(h, "loadedCount", 4);
        compare(h.overflow.length, 0);
        verify(!h.moreShown);
        verify(h.slotAt(0) === first, "delegates kept");
    }

    function test_apps_group_hidden_when_empty() {
        var g = createTemporaryObject(appsComp, tc, { bridge: fake });
        compare(g.loadedCount, 0);
        verify(!g.shown);
        // A bar widget doesn't count; a quick one that loads does.
        fake.widgets = [entry("b", "good", "bar"), entry("q", "good", "quick")];
        tryCompare(g, "loadedCount", 1);
        verify(g.shown);
        fake.widgets = [entry("b", "good", "bar")];
        tryCompare(g, "loadedCount", 0);
        verify(!g.shown);
    }

    function test_apps_group_hidden_when_its_only_tile_fails() {
        ignoreWarning(/.*/);
        fake.widgets = [entry("brk", "broken", "quick")];
        var g = createTemporaryObject(appsComp, tc, { bridge: fake });
        tryVerify(function () { return Widgets.log.length === 1; });
        compare(g.loadedCount, 0);
        verify(!g.shown);
    }
}
