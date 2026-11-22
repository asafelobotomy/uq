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

    function test_date_format() {
        compare(ShellPrefs.dateFormat(undefined), "uk");
        compare(ShellPrefs.dateFormat({}), "uk");
        compare(ShellPrefs.dateFormat({ date_format: "iso" }), "uk");
        compare(ShellPrefs.dateFormat({ date_format: "us" }), "us");
        compare(ShellPrefs.datePatterns({}).short, "ddd d MMM");
        compare(ShellPrefs.datePatterns({ date_format: "us" }).short, "ddd, MMM d");
        compare(ShellPrefs.datePatterns({ date_format: "us" }).locale, "en_US");
        compare(Qt.locale(ShellPrefs.datePatterns({}).locale).firstDayOfWeek, Locale.Monday);
        compare(Qt.locale(ShellPrefs.datePatterns({ date_format: "us" }).locale).firstDayOfWeek, Locale.Sunday);
        var d = new Date(2026, 10, 21);
        compare(Qt.formatDate(d, ShellPrefs.datePatterns({ date_format: "us" }).full), "Saturday, November 21, 2026");
        compare(Qt.formatDate(d, ShellPrefs.datePatterns({}).full), "Saturday 21 November 2026");
    }

    function test_temp_unit() {
        compare(ShellPrefs.tempUnit(undefined), "c");
        compare(ShellPrefs.tempUnit({ temp_unit: "k" }), "c");
        compare(ShellPrefs.tempUnit({ temp_unit: "f" }), "f");
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

    function test_provider_enabled() {
        verify(ShellPrefs.providerEnabled(undefined, "windows"));
        verify(ShellPrefs.providerEnabled({ providers: [] }, "windows"));
        verify(!ShellPrefs.providerEnabled({ providers: [{ id: "windows", enabled: false }] }, "windows"));
        verify(ShellPrefs.providerEnabled({ providers: [{ id: "windows", enabled: false }] }, "files"));
    }
}
