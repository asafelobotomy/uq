import QtQuick
import QtTest
import "../../shell/logic/launcher.js" as L

TestCase {
    name: "launcher"

    // Enter inside the 70 ms debounce must not act on the previous query's rows: a `>` row would
    // run the old command. Stale when a search is pending or the field no longer matches what
    // the shown rows were searched for.
    function test_results_stale() {
        compare(L.resultsStale(false, "fire", "fire"), false);
        compare(L.resultsStale(false, "  fire ", "fire"), false);
        compare(L.resultsStale(true, "fire", "fire"), true);
        compare(L.resultsStale(false, "> rm -rf ~/build", "> rm -rf ~/"), true);
        compare(L.resultsStale(false, "firefox", "fi"), true);
        compare(L.resultsStale(false, "", ""), false);
        compare(L.resultsStale(false, undefined, ""), false);
    }

    function test_classify() {
        compare(L.classify("=1+1"), "calc");
        compare(L.classify("  =1"), "calc");
        compare(L.classify(">ls -la"), "run");
        compare(L.classify(" > ls"), "run");
        compare(L.classify(":smile"), "emoji");
        compare(L.classify("firefox"), "search");
        compare(L.classify("1+1"), "search");
        compare(L.classify(""), "search");
        compare(L.classify(null), "search");
        compare(L.classify(undefined), "search");
    }

    function test_next_generation() {
        compare(L.nextGeneration(0), 1);
        compare(L.nextGeneration(41), 42);
        compare(L.nextGeneration(undefined), 1);
    }

    function test_state_machine_single_surface() {
        var s = L.initialState();
        compare(s.open, "closed");
        s = L.transition(s, "toggleSpotlight");
        compare(s.open, "spotlight");
        verify(!s.wipe);
        // Opening one surface closes the other, which wipes.
        s = L.transition(s, "toggleAppMenu");
        compare(s.open, "appmenu");
        verify(s.wipe);
        s = L.transition(s, "toggleSpotlight");
        compare(s.open, "spotlight");
        verify(s.wipe);
        // Toggling the open surface closes it.
        s = L.transition(s, "toggleSpotlight");
        compare(s.open, "closed");
        verify(s.wipe);
        s = L.transition(s, "toggleAppMenu");
        compare(s.open, "appmenu");
        verify(!s.wipe);
        s = L.transition(s, "close");
        compare(s.open, "closed");
        verify(s.wipe);
        // Quick settings is a third surface: opening it closes Spotlight and reports the wipe.
        s = L.transition({ open: "spotlight", wipe: false }, "toggleQuickSettings");
        compare(s.open, "quicksettings");
        verify(s.wipe);
        s = L.transition(s, "toggleAppMenu");
        compare(s.open, "appmenu");
        verify(s.wipe);
        s = L.transition(s, "toggleQuickSettings");
        compare(s.open, "quicksettings");
        s = L.transition(s, "toggleQuickSettings");
        compare(s.open, "closed");
        verify(s.wipe);
        s = L.transition(L.initialState(), "toggleQuickSettings");
        compare(s.open, "quicksettings");
        verify(!s.wipe);
        s = L.transition(s, "close");
        compare(s.open, "closed");
        verify(s.wipe);
        // Unknown events and a bad state leave things closed or unchanged.
        compare(L.transition(L.transition(L.initialState(), "toggleSpotlight"), "bogus").open,
                "spotlight");
        compare(L.transition(null, "bogus").open, "closed");
        compare(L.transition({ open: "weird" }, "close").open, "closed");
    }

    function test_close_and_wipe_on_lock() {
        var states = ["closed", "spotlight", "appmenu", "quicksettings"];
        for (var i = 0; i < states.length; i++) {
            var s = L.transition({ open: states[i], wipe: false }, "lock");
            compare(s.open, "closed", states[i]);
            verify(s.wipe, states[i]);
        }
    }

    // The built-in notification centre is a fourth surface: it toggles, opening it closes (and
    // wipes) whatever else is open, opening another surface closes it, and lock wipes it.
    function test_transition_notifcentre() {
        var s = L.transition(L.initialState(), "toggleNotifCentre");
        compare(s.open, "notifcentre");
        verify(!s.wipe);
        s = L.transition(s, "toggleNotifCentre");
        compare(s.open, "closed");
        verify(s.wipe);
        var others = ["spotlight", "appmenu", "quicksettings"];
        for (var i = 0; i < others.length; i++) {
            s = L.transition({ open: others[i], wipe: false }, "toggleNotifCentre");
            compare(s.open, "notifcentre", others[i]);
            verify(s.wipe, others[i]);
        }
        s = L.transition({ open: "notifcentre", wipe: false }, "toggleQuickSettings");
        compare(s.open, "quicksettings");
        verify(s.wipe);
        s = L.transition({ open: "notifcentre", wipe: false }, "close");
        compare(s.open, "closed");
        verify(s.wipe);
        s = L.transition({ open: "notifcentre", wipe: false }, "lock");
        compare(s.open, "closed");
        verify(s.wipe);
    }

    // The power menu is a fifth surface, toggled like the others and wiped on lock.
    function test_transition_powermenu() {
        compare(L.SURFACES.indexOf("powermenu") >= 0, true);
        var s = L.transition(L.initialState(), "togglePowerMenu");
        compare(s.open, "powermenu");
        verify(!s.wipe);
        s = L.transition(s, "togglePowerMenu");
        compare(s.open, "closed");
        verify(s.wipe);
        var others = ["spotlight", "appmenu", "quicksettings", "notifcentre"];
        for (var i = 0; i < others.length; i++) {
            s = L.transition({ open: others[i], wipe: false }, "togglePowerMenu");
            compare(s.open, "powermenu", others[i]);
            verify(s.wipe, others[i]);
        }
        s = L.transition({ open: "powermenu", wipe: false }, "toggleSpotlight");
        compare(s.open, "spotlight");
        verify(s.wipe);
        s = L.transition({ open: "powermenu", wipe: false }, "close");
        compare(s.open, "closed");
        verify(s.wipe);
        s = L.transition({ open: "powermenu", wipe: false }, "lock");
        compare(s.open, "closed");
        verify(s.wipe);
        // An unknown event leaves the power menu open.
        s = L.transition({ open: "powermenu", wipe: false }, "bogus");
        compare(s.open, "powermenu");
    }

    // The desktop menu is one more surface: toggled like the others, replaced by them, wiped on lock.
    function test_transition_desktopmenu() {
        verify(L.SURFACES.indexOf("desktopmenu") >= 0);
        var s = L.transition(L.initialState(), "toggleDesktopMenu");
        compare(s.open, "desktopmenu");
        verify(!s.wipe);
        s = L.transition(s, "toggleDesktopMenu");
        compare(s.open, "closed");
        verify(s.wipe);
        s = L.transition({ open: "desktopmenu", wipe: false }, "togglePowerMenu");
        compare(s.open, "powermenu");
        verify(s.wipe);
        s = L.transition({ open: "spotlight", wipe: false }, "toggleDesktopMenu");
        compare(s.open, "desktopmenu");
        verify(s.wipe);
        s = L.transition({ open: "desktopmenu", wipe: false }, "lock");
        compare(s.open, "closed");
        verify(s.wipe);
    }

    // Sections: top hit, then two groups (2 and 3 items).
    function sections() {
        return [
            { provider: "apps", top: true, items: [{ id: "t" }] },
            { provider: "apps", items: [{ id: "a1" }, { id: "a2" }] },
            { provider: "calc", items: [{ id: "c1" }, { id: "c2" }, { id: "c3" }] }
        ];
    }

    function test_move_wraps_and_tab_groups() {
        var g = sections();
        var s = { group: 0, index: 0 };
        s = L.move(s, g, "down");
        compare(s.group, 1); compare(s.index, 0);
        s = L.move(s, g, "down");
        compare(s.group, 1); compare(s.index, 1);
        s = L.move(s, g, "down");
        compare(s.group, 2); compare(s.index, 0);
        s = L.move({ group: 2, index: 2 }, g, "down");
        compare(s.group, 0); compare(s.index, 0);      // wraps to the top
        s = L.move({ group: 0, index: 0 }, g, "up");
        compare(s.group, 2); compare(s.index, 2);      // wraps to the bottom
        s = L.move({ group: 2, index: 0 }, g, "up");
        compare(s.group, 1); compare(s.index, 1);

        s = L.move({ group: 1, index: 1 }, g, "tab");
        compare(s.group, 2); compare(s.index, 0);
        s = L.move(s, g, "tab");
        compare(s.group, 0); compare(s.index, 0);      // wraps
        s = L.move({ group: 2, index: 2 }, g, "backtab");
        compare(s.group, 1); compare(s.index, 0);
        s = L.move({ group: 0, index: 0 }, g, "backtab");
        compare(s.group, 2); compare(s.index, 0);      // wraps

        // Empty sections are skipped; an empty list has no selection.
        var h = [{ items: [{ id: "x" }] }, { items: [] }, { items: [{ id: "y" }] }];
        s = L.move({ group: 0, index: 0 }, h, "tab");
        compare(s.group, 2); compare(s.index, 0);
        s = L.move({ group: 0, index: 0 }, [], "down");
        compare(s.group, -1); compare(s.index, -1);
        // An out-of-range selection is clamped before moving.
        s = L.move({ group: 9, index: 9 }, g, "down");
        compare(s.group, 0); compare(s.index, 0);
        s = L.move({ group: 1, index: 0 }, g, "left");
        compare(s.group, 1); compare(s.index, 0);
    }

    function test_clamp_selection() {
        var g = sections();
        var s = L.clampSelection({ group: 1, index: 5 }, g);
        compare(s.group, 1); compare(s.index, 1);
        s = L.clampSelection(null, g);
        compare(s.group, 0); compare(s.index, 0);
        s = L.clampSelection({ group: 7, index: 0 }, g);
        compare(s.group, 0); compare(s.index, 0);
        s = L.clampSelection({ group: 0, index: 0 }, []);
        compare(s.group, -1);
    }

    function test_sections_move_top_hit_out_of_its_group() {
        var app = { id: "firefox.desktop", name: "Firefox", score: 1, kind: "exact" };
        var merged = {
            topHit: { provider: "apps", providerName: "Apps", item: app },
            groups: [
                { provider: "apps", providerName: "Apps", items: [app] },
                { provider: "calc", providerName: "Calculator", items: [{ id: "calc", title: "2" }] }
            ]
        };
        var s = L.sections(merged);
        compare(s.length, 2);          // the Apps group is empty once its only item is the top hit
        verify(s[0].top);
        compare(s[0].items[0].id, "firefox.desktop");
        compare(s[0].provider, "apps");
        compare(s[1].provider, "calc");
        compare(s[1].providerName, "Calculator");
        compare(L.sections({ topHit: null, groups: [] }).length, 0);
        compare(L.sections(null).length, 0);
        // A provider item with the same id as an app is not taken out of the Apps group.
        var m2 = {
            topHit: { provider: "x.y", providerName: "X", item: { id: "a" } },
            groups: [{ provider: "apps", providerName: "Apps", items: [{ id: "a" }] },
                     { provider: "x.y", providerName: "X", items: [{ id: "a" }, { id: "b" }] }]
        };
        var s2 = L.sections(m2);
        compare(s2.length, 3);
        compare(s2[1].items.length, 1);
        compare(s2[2].items.length, 1);
        compare(s2[2].items[0].id, "b");
    }

    function test_activation() {
        var a = L.activation("apps", { id: "firefox.desktop" });
        compare(a.kind, "launch");
        compare(a.call, "Launch");
        compare(a.args[0].desktop_id, "firefox.desktop");
        verify(a.close);

        a = L.activation("calc", { id: "calc", actions: ["copy"] });
        compare(a.kind, "copy");
        compare(a.call, "Activate");
        compare(JSON.stringify(a.args), JSON.stringify(["calc", "calc", "copy"]));
        verify(!a.close);

        a = L.activation("run", { id: "run-7" });
        compare(a.kind, "launch");
        compare(a.call, "Launch");
        compare(a.args[0].run_id, "run-7");
        compare(a.args[0].desktop_id, undefined);
        verify(a.close);

        a = L.activation("org.x.Emoji", { id: "e1", actions: ["insert", "other"] });
        compare(a.kind, "activate");
        compare(a.call, "Activate");
        compare(JSON.stringify(a.args), JSON.stringify(["org.x.Emoji", "e1", "insert"]));
        verify(a.close);
        a = L.activation("org.x.Emoji", { id: "e1" });
        compare(a.args[2], "");

        compare(L.activation("apps", null), null);
        compare(L.activation("apps", { id: "" }), null);
        compare(L.activation("", { id: "x" }), null);
    }

    function test_copy_value() {
        compare(L.copyValue({ copy: "42" }), "42");
        compare(L.copyValue({ copy: 3 }), "3");
        compare(L.copyValue({}), null);
        compare(L.copyValue(null), null);
        compare(L.copyValue("42"), null);
    }

    function test_icon_ref() {
        compare(JSON.stringify(L.iconRef("firefox")), JSON.stringify({ theme: "firefox" }));
        compare(JSON.stringify(L.iconRef("org.gnome.Nautilus-symbolic")),
                JSON.stringify({ theme: "org.gnome.Nautilus-symbolic" }));
        compare(JSON.stringify(L.iconRef("/usr/share/icons/a b#?.png")),
                JSON.stringify({ path: "file:///usr/share/icons/a%20b%23%3F.png" }));
        // Never a URL: anything else falls back to the default theme icon.
        var bad = ["https://evil.example/x.png", "file:///etc/x.png", "//host/x.png", "qrc:/x",
                   "image://x/y", "", "..", ".", "a b", "a/b", null, undefined, 5, "data:image/png;base64,AA"];
        for (var i = 0; i < bad.length; i++)
            compare(JSON.stringify(L.iconRef(bad[i])), JSON.stringify({ theme: "system-search" }),
                    String(bad[i]));
    }

    function test_emoji_hint() {
        // Shown only for a `:` query with text, after the deadline, with no external results.
        verify(L.showEmojiHint(":smile", false, true));
        verify(!L.showEmojiHint(":smile", true, true));
        verify(!L.showEmojiHint(":smile", false, false));
        verify(!L.showEmojiHint(":", false, true));
        verify(!L.showEmojiHint(":  ", false, true));
        verify(!L.showEmojiHint("smile", false, true));
    }

    function test_app_entries() {
        var e = L.appEntries([
            { id: "a.desktop", name: "A", genericName: "G", comment: "C", icon: "a",
              keywords: "one;two;", noDisplay: false, extra: 1 },
            { id: "b.desktop", name: "B", noDisplay: true },
            null
        ]);
        compare(e.length, 2);
        compare(e[0].id, "a.desktop");
        compare(JSON.stringify(e[0].keywords), JSON.stringify(["one", "two"]));
        compare(e[0].extra, undefined);
        verify(e[1].noDisplay);
        compare(JSON.stringify(e[1].keywords), JSON.stringify([]));
        compare(L.appEntries(null).length, 0);
    }
}
