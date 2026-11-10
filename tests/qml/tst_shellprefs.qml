import QtQuick
import QtTest
import "../../shell/logic/shellprefs.js" as ShellPrefs

TestCase {
    name: "shellprefs"

    function test_island_defaults_on() {
        var ids = ["system", "work", "time", "notif", "status"];
        for (var i = 0; i < ids.length; i++) {
            verify(ShellPrefs.islandOn(undefined, ids[i]));
            verify(ShellPrefs.islandOn(null, ids[i]));
            verify(ShellPrefs.islandOn({}, ids[i]));
            verify(ShellPrefs.islandOn({ islands: {} }, ids[i]));
            verify(ShellPrefs.islandOn({ islands: { work: false } }, ids[i]) === (ids[i] !== "work"));
        }
        verify(!ShellPrefs.islandOn({ islands: { time: false } }, "time"));
        verify(ShellPrefs.islandOn({ islands: { time: true } }, "time"));
        verify(ShellPrefs.islandOn({ islands: null }, "time"));
    }

    function test_clock_format() {
        compare(ShellPrefs.clockFormat(undefined), "HH:mm");
        compare(ShellPrefs.clockFormat(null), "HH:mm");
        compare(ShellPrefs.clockFormat({}), "HH:mm");
        compare(ShellPrefs.clockFormat({ clock_24h: true }), "HH:mm");
        compare(ShellPrefs.clockFormat({ clock_24h: false }), "h:mm AP");
    }

    function test_provider_order() {
        compare(ShellPrefs.providerOrder(undefined), []);
        compare(ShellPrefs.providerOrder({}), []);
        compare(ShellPrefs.providerOrder({ providers: null }), []);
        compare(ShellPrefs.providerOrder({ providers: [
            { id: "run", enabled: true },
            { id: "emoji", enabled: false },
            { id: "calc", enabled: true }
        ] }), ["run", "calc"]);
    }
}
