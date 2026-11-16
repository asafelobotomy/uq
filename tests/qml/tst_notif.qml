import QtQuick
import QtTest
import "../../shell/logic/notif.js" as N

TestCase {
    name: "notif"

    function env(over) {
        var e = { notifi: false, graceOver: false, detected: { swaync: null, mako: null, dunst: null } };
        for (var k in over)
            e[k] = over[k];
        return e;
    }

    function test_clean_text() {
        compare(N.cleanText(42, 10), "");
        compare(N.cleanText(null, 10), "");
        compare(N.cleanText("  hi\u0000there\u007f\n", 20), "hi there");
        compare(N.cleanText("a\u202Eb\u2066c\u2069d", 20), "a b c d");
        compare(N.cleanText("a\u0080b\u009fc\u2028d\u2029e\u0085f", 20), "a b c d e f");
        compare(N.cleanText("\u00a0x\u00a0", 20), "x");
        compare(N.cleanText("abcdefghij", 5), "abcd…");
        compare(N.cleanText("abcde", 5), "abcde");
    }

    function test_choose_backend_auto() {
        compare(N.chooseBackend("auto", env({ notifi: true })).backend, "notifi");
        compare(N.chooseBackend("bogus", env({ notifi: true })).backend, "notifi");
        compare(N.chooseBackend("auto", env({})).backend, "pending");
        var r = N.chooseBackend("auto", env({ graceOver: true }));
        compare(r.backend, "builtin");
        compare(r.warning, "");
        compare(N.chooseBackend("builtin", env({ notifi: true })).backend, "builtin");
    }

    function test_choose_backend_grace() {
        compare(N.chooseBackend("notifi", env({})).backend, "pending");
        compare(N.chooseBackend("notifi", env({ notifi: true })).backend, "notifi");
        compare(N.chooseBackend("notifi", env({ notifi: true })).warning, "");
        var r = N.chooseBackend("notifi", env({ graceOver: true }));
        compare(r.backend, "builtin");
        compare(r.warning, "noti-fi is not running");
        compare(N.NOTIFI_GRACE_MS, 5000);
    }

    function test_choose_backend_missing() {
        var names = ["swaync", "mako", "dunst"];
        for (var i = 0; i < names.length; i++) {
            var d = { swaync: null, mako: null, dunst: null };
            compare(N.chooseBackend(names[i], env({ detected: d })).backend, "pending");
            d[names[i]] = true;
            compare(N.chooseBackend(names[i], env({ detected: d })).backend, names[i]);
            d[names[i]] = false;
            var r = N.chooseBackend(names[i], env({ detected: d }));
            compare(r.backend, "builtin");
            compare(r.warning, names[i] + " is not installed");
        }
    }

    function test_builtin_should_register() {
        verify(!N.builtinShouldRegister("builtin", { known: false, owner: null, name: "", self: false }));
        verify(N.builtinShouldRegister("builtin", { known: true, owner: null, name: "", self: false }));
        verify(!N.builtinShouldRegister("builtin", { known: true, owner: ":1.5", name: "dunst", self: false }));
        verify(N.builtinShouldRegister("builtin", { known: true, owner: ":1.5", name: "uq", self: true }));
        verify(!N.builtinShouldRegister("mako", { known: true, owner: null, name: "", self: false }));
    }

    function test_count_label() {
        compare(N.countLabel(0), "");
        compare(N.countLabel(1), "1");
        compare(N.countLabel(99), "99");
        compare(N.countLabel(100), "99+");
        compare(N.countLabel(NaN), "");
        compare(N.countLabel(-3), "");
    }

    function backendView(over) {
        var b = { available: true, count: 2, latest: { appIcon: "x", appName: "App", summary: "Hello" }, dnd: false, sharing: false };
        for (var k in over)
            b[k] = over[k];
        return b;
    }

    function test_island_view_sharing() {
        var o = { sharing: false, showSummary: true, hovered: true, yieldedTo: "" };
        compare(N.islandView(backendView({}), o).summary, "Hello");
        compare(N.islandView(backendView({ sharing: true }), o).summary, "");
        o.sharing = true;
        compare(N.islandView(backendView({}), o).summary, "");
        o.hovered = false;
        compare(N.islandView(backendView({}), o).summary, "");
    }

    function test_island_view_summary() {
        var o = { sharing: false, showSummary: false, hovered: false, yieldedTo: "" };
        compare(N.islandView(backendView({}), o).summary, "");
        o.hovered = true;
        compare(N.islandView(backendView({}), o).summary, "Hello");
        o.hovered = false;
        o.showSummary = true;
        var v = N.islandView(backendView({ dnd: true }), o);
        compare(v.summary, "Hello");
        compare(v.count, 2);
        compare(v.appIcon, "x");
        compare(v.dnd, true);
        compare(v.yieldedTo, "");
        o.yieldedTo = "dunst";
        compare(N.islandView(backendView({}), o).yieldedTo, "Notifications handled by dunst");
        o.yieldedTo = "";
        compare(N.islandView(backendView({}), { yieldedTo: null }).yieldedTo, "");
        compare(N.islandView(backendView({ latest: null }), o).summary, "");
    }

    function test_parse_swaync() {
        var a = N.parseSwaync('{"count":3,"dnd":true,"visible":false}');
        compare(a.count, 3);
        compare(a.dnd, true);
        compare(a.centre, false);
        var b = N.parseSwaync('{"text":"3","alt":"dnd-notification","tooltip":"3 Notifications","class":["notification"]}');
        compare(b.count, 3);
        compare(b.dnd, true);
        compare(b.centre, false);
        var c = N.parseSwaync('{"text":"0","alt":"none","class":["none"]}');
        compare(c.count, 0);
        compare(c.dnd, false);
        compare(N.parseSwaync('{"text":"1","alt":"inhibited-notification"}').dnd, false);
        var z = N.parseSwaync('{"text":"","alt":"none"}');
        compare(z.count, 0);
        compare(z.dnd, false);
        compare(N.parseSwaync('{"text":"abc","alt":"none"}'), null);
        compare(N.parseSwaync("garbage"), null);
        compare(N.parseSwaync("[1]"), null);
        compare(N.parseSwaync('{"foo":1}'), null);
        compare(N.parseSwaync(""), null);
        compare(N.parseSwaync(5), null);
    }

    function test_parse_mako_text() {
        var out = "Notification 12: Build finished\n  App name: make\n  Category: x\nNotification 11: Older\n  App name: other\n";
        var r = N.parseMakoList(out);
        compare(r.count, 2);
        compare(r.latest.summary, "Build finished");
        compare(r.latest.appName, "make");
        compare(r.latest.appIcon, "");
        var e = N.parseMakoList("");
        compare(e.count, 0);
        compare(e.latest, null);
        var s = N.parseMakoList("Notification 1: Hi\u202E\n");
        compare(s.count, 1);
        compare(s.latest.summary, "Hi");
        compare(s.latest.appName, "");
    }

    function test_parse_mako_json() {
        var out = '{"type":"aa{sv}","data":[[{"app-name":{"type":"s","data":"x"},"summary":{"type":"s","data":"y"},"id":{"type":"u","data":3}},{"app-name":{"type":"s","data":"z"},"summary":{"type":"s","data":"w"}}]]}';
        var r = N.parseMakoList(out);
        compare(r.count, 2);
        compare(r.latest.appName, "x");
        compare(r.latest.summary, "y");
        var e = N.parseMakoList('{"type":"aa{sv}","data":[[]]}');
        compare(e.count, 0);
        compare(e.latest, null);
        compare(N.parseMakoList('{"data":5}').count, 0);
    }

    function test_parse_mako_modes() {
        verify(N.parseMakoModes("default\ndo-not-disturb\n"));
        verify(!N.parseMakoModes("default\n"));
        verify(!N.parseMakoModes("do-not-disturb-ish\n"));
        verify(!N.parseMakoModes(""));
        verify(!N.parseMakoModes(null));
    }

    function test_parse_dunst() {
        var out = "              Waiting: 1\n  Currently displayed: 2\n              History: 5\n";
        compare(N.parseDunstCount(out), 3);
        compare(N.parseDunstCount("junk"), 0);
        compare(N.parseDunstCount("Waiting: " + "9".repeat(400) + "\n"), 0);
        compare(N.parseDunstCount(""), 0);
        compare(N.parseDunstCount(null), 0);
        compare(N.parseDunstCount("Waiting: x\nCurrently displayed: 2\n"), 2);
        verify(N.parseDunstPaused("true\n"));
        verify(!N.parseDunstPaused("false\n"));
        verify(!N.parseDunstPaused(""));
    }
    function test_parse_version() {
        compare(N.parseVersion("1.13.2 (2026-03-18)"), [1, 13, 2]);
        compare(N.parseVersion("v1.13.1-12-gabc123"), [1, 13, 1]);
        compare(N.parseVersion("1.10"), [1, 10, 0]);
        compare(N.parseVersion("dunst"), null);
        compare(N.parseVersion(""), null);
        compare(N.parseVersion(null), null);
        verify(N.versionAtLeast([1, 13, 1], [1, 13, 1]));
        verify(N.versionAtLeast([1, 14, 0], [1, 13, 1]));
        verify(N.versionAtLeast([2, 0, 0], [1, 13, 1]));
        verify(!N.versionAtLeast([1, 13, 0], [1, 13, 1]));
        verify(!N.versionAtLeast([1, 9, 9], [1, 10, 0]));
        verify(!N.versionAtLeast(null, [1, 0, 0]));
    }

    function test_parse_server_info() {
        var i = N.parseServerInfo("('dunst', 'knopwob', '1.13.2 (2026-03-18)', '1.2')\n");
        compare(i.name, "dunst");
        compare(i.vendor, "knopwob");
        compare(i.version, "1.13.2 (2026-03-18)");
        compare(N.parseServerInfo("('dunst', 'knopwob')"), null);
        compare(N.parseServerInfo("junk"), null);
        compare(N.parseServerInfo(null), null);
        compare(N.parseServerInfo("('" + "x".repeat(5000) + "', 'a', '1.0', '1.2')"), null);
    }

    function test_dunst_events() {
        function info(v) { return "('dunst', 'knopwob', '" + v + "', '1.2')"; }
        compare(N.dunstEvents(info("1.13.2 (2026-03-18)")), { dnd: true, count: true });
        compare(N.dunstEvents(info("1.13.1")), { dnd: true, count: true });
        compare(N.dunstEvents(info("v1.13.0-40-gdeadbee")), { dnd: false, count: true });
        compare(N.dunstEvents(info("1.10.0")), { dnd: false, count: true });
        compare(N.dunstEvents(info("1.9.2")), { dnd: false, count: false });
        compare(N.dunstEvents(info("unknown")), { dnd: false, count: false });
        compare(N.dunstEvents("('mako', 'emersion', '1.13.2', '1.2')"), { dnd: false, count: false });
        compare(N.dunstEvents(""), { dnd: false, count: false });
    }

    function test_mako_events() {
        function xml(props) {
            return "('<!DOCTYPE node>\\n<node>\\n <interface name=\"org.freedesktop.DBus.Peer\">\\n"
                + " <property name=\"Modes\" type=\"as\" access=\"read\"/></interface>\\n"
                + " <interface name=\"fr.emersion.Mako\">\\n  <method name=\"ListModes\"/>\\n"
                + props + " </interface>\\n</node>\\n',)";
        }
        var modes = "  <property name=\"Modes\" type=\"as\" access=\"read\">\\n   <annotation/>\\n  </property>\\n";
        var notifs = "  <property name=\"Notifications\" type=\"aa{sv}\" access=\"read\">\\n  </property>\\n";
        compare(N.makoEvents(xml(modes + notifs)), { dnd: true, count: true });
        compare(N.makoEvents(xml(modes)), { dnd: true, count: false });
        compare(N.makoEvents(xml("")), { dnd: false, count: false });
        // Only the fr.emersion.Mako interface counts.
        compare(N.makoEvents("<interface name=\"x\"><property name=\"Modes\" type=\"as\"/></interface>"),
            { dnd: false, count: false });
        compare(N.makoEvents(null), { dnd: false, count: false });
    }

    function test_monitor_line() {
        var name = "org.freedesktop.Notifications";
        var path = "/org/freedesktop/Notifications";
        var iface = "org.dunstproject.cmd0";
        compare(N.monitorLine("The name " + name + " is owned by :1.42", name, path, iface), "owner");
        compare(N.monitorLine("The name " + name + " does not have an owner", name, path, iface), "lost");
        compare(N.monitorLine(path + ": org.freedesktop.DBus.Properties.PropertiesChanged ('" + iface
            + "', {'paused': <true>, 'pauseLevel': <uint32 100>}, @as [])", name, path, iface), "change");
        compare(N.monitorLine("/fr/emersion/Mako: org.freedesktop.DBus.Properties.PropertiesChanged "
            + "('fr.emersion.Mako', @a{sv} {}, ['Modes'])", name, "/fr/emersion/Mako", "fr.emersion.Mako"), "change");
        // Other interfaces, signals and paths are not changes.
        compare(N.monitorLine(path + ": org.freedesktop.DBus.Properties.PropertiesChanged ('other', {}, @as [])",
            name, path, iface), null);
        compare(N.monitorLine(path + ": org.freedesktop.Notifications.NotificationClosed (uint32 3, uint32 2)",
            name, path, iface), null);
        compare(N.monitorLine("/elsewhere: org.freedesktop.DBus.Properties.PropertiesChanged ('" + iface + "', {}, @as [])",
            name, path, iface), null);
        compare(N.monitorLine("Monitoring signals on object " + path + " owned by " + name, name, path, iface), null);
        compare(N.monitorLine("The name other.Name is owned by :1.2", name, path, iface), null);
        compare(N.monitorLine(path + ": org.freedesktop.DBus.Properties.PropertiesChanged ('" + iface + "', "
            + "x".repeat(5000), name, path, iface), null);
        compare(N.monitorLine(null, name, path, iface), null);
    }

    function test_poll_needs() {
        compare(N.pollNeeds({ dnd: true, count: true }, true), { dnd: false, count: false });
        compare(N.pollNeeds({ dnd: true, count: false }, true), { dnd: false, count: true });
        compare(N.pollNeeds({ dnd: true, count: true }, false), { dnd: true, count: true });
        compare(N.pollNeeds(N.NO_EVENTS, true), { dnd: true, count: true });
        compare(N.pollNeeds(null, true), { dnd: true, count: true });
    }
}
