import QtQuick
import QtTest
import "../../shell/logic/outputs.js" as O
import "../../shell/core"

TestCase {
    name: "outputs"

    function screens() {
        return [{ name: "DP-1" }, { name: "HDMI-A-1" }, { name: "eDP-1" }];
    }

    function test_primary_found() {
        var s = screens();
        verify(O.primaryScreen(s, { primary: "HDMI-A-1", main: "HDMI-A-1", bar: null }) === s[1]);
        // Not a wrapper: the very ShellScreen object.
        verify(O.primaryScreen(s, { primary: "eDP-1" }) === s[2]);
    }

    function test_primary_empty_is_null() {
        var s = screens();
        compare(O.primaryScreen(s, { primary: "", main: "DP-1", bar: null }), null);
        compare(O.primaryScreen(s, null), null);
        compare(O.primaryScreen(s, {}), null);
        compare(O.primaryScreen(s, { primary: "DP-9" }), null);
        compare(O.primaryScreen(null, { primary: "DP-1" }), null);
    }

    function test_bar_all_when_null() {
        var s = screens();
        var a = O.barScreens(s, null);
        compare(a.length, 3);
        var b = O.barScreens(s, { primary: "", main: "DP-1", bar: null });
        compare(b.length, 3);
        for (var i = 0; i < 3; i++) {
            verify(a[i] === s[i]);
            verify(b[i] === s[i]);
        }
        compare(O.barScreens([], null).length, 0);
    }

    function test_bar_chosen() {
        var s = screens();
        var r = O.barScreens(s, { primary: "", main: "DP-1", bar: ["eDP-1", "DP-1"] });
        compare(r.length, 2);
        // Screen order, and the screen objects themselves (Variants keeps delegates by identity).
        verify(r[0] === s[0]);
        verify(r[1] === s[2]);
    }

    function test_bar_never_empty() {
        var s = screens();
        var r = O.barScreens(s, { primary: "", main: "", bar: ["DP-9"] });
        compare(r.length, 3);
        compare(O.barScreens(s, { bar: [] }).length, 3);
        compare(O.barScreens(s, { bar: "DP-1" }).length, 3);
    }

    function test_singleton_follows_roles() {
        var s = screens();
        Outputs.roles = null;
        compare(Outputs.barScreens(s).length, 3);
        compare(Outputs.primaryScreen(s), null);
        Outputs.roles = { primary: "DP-1", main: "DP-1", bar: ["DP-1"] };
        compare(Outputs.barScreens(s).length, 1);
        verify(Outputs.primaryScreen(s) === s[0]);
        Outputs.roles = null;
    }
}
