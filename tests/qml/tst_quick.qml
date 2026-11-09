import QtQuick
import QtTest
import "../../shell/logic/quick.js" as Q

TestCase {
    name: "quick"

    function test_volume_step_clamps() {
        compare(Q.volumeStep(0.98, 1), 1);
        compare(Q.volumeStep(1, 1), 1);
        compare(Q.volumeStep(0.02, -1), 0);
        compare(Q.volumeStep(0, -1), 0);
        compare(Q.volumeStep(0.5, 1), 0.55);
        compare(Q.volumeStep(NaN, 1), 0);
        compare(Q.volumeStep(undefined, -1), 0);
    }

    function test_osd_should_show() {
        compare(Q.osdShouldShow(0, 1999, 5, 5), false);
        compare(Q.osdShouldShow(0, 2000, 5, 5), true);
        compare(Q.osdShouldShow(0, 9000, 5, 6), false);
        compare(Q.osdShouldShow(0, 9000, 5, 5), true);
    }

    function test_brightness_parse() {
        compare(Q.brightnessParse("intel_backlight,backlight,4800,50%,9600\n"),
                {device: "intel_backlight", current: 4800, max: 9600, percent: 50});
        compare(Q.brightnessParse("input0::capslock,leds,0,0%,1"), null);
        compare(Q.brightnessParse(""), null);
        compare(Q.brightnessParse(null), null);
        compare(Q.brightnessParse("x,backlight,1,1%,0"), null);
        compare(Q.brightnessParse("garbage"), null);
        compare(Q.brightnessParse("x,backlight,a,1%,5"), null);
        compare(Q.brightnessParse("x,backlight,1,1,5"), null);
        // first backlight line wins, leds skipped
        compare(Q.brightnessParse("k,leds,0,0%,1\nb,backlight,10,100%,10\nc,backlight,1,10%,10\n").device, "b");
    }

    function test_brightness_args() {
        compare(Q.brightnessArgs(1), ["brightnessctl", "-m", "-e4", "-n2", "set", "5%+"]);
        compare(Q.brightnessArgs(-1), ["brightnessctl", "-m", "-e4", "-n2", "set", "5%-"]);
        compare(Q.BRIGHTNESS_READ, ["brightnessctl", "-m"]);
    }

    function test_wifi_rows_and_connectable() {
        var rows = Q.wifiRows([
            {name: "far", signal: 0.2, connected: false, known: false, secure: false},
            {name: "known", signal: 0.4, connected: false, known: true, secure: true},
            {name: "", signal: 1, connected: false, known: true, secure: false},
            {name: "home", signal: 50, connected: true, known: true, secure: true},
            {name: "locked", signal: 90, connected: false, known: false, secure: true},
            {name: "knownstrong", signal: 80, connected: false, known: true, secure: true}
        ]);
        compare(rows.length, 5);
        compare(rows.map(function (r) { return r.name; }),
                ["home", "knownstrong", "known", "locked", "far"]);
        compare(rows[0].connectable, false);
        compare(rows[1].connectable, true);
        compare(rows[3].connectable, false);   // unknown + secured
        compare(rows[4].connectable, true);    // unknown + open
        compare(rows[2].signal, 40);
        compare(rows[1].signal, 80);
        var many = [];
        for (var i = 0; i < 25; i++)
            many.push({name: "n" + i, signal: i, connected: false, known: false, secure: false});
        compare(Q.wifiRows(many).length, 20);
        compare(Q.wifiRows(null).length, 0);
    }

    function test_schedule_label_and_next() {
        compare(Q.scheduleLabel({mode: "off"}), "Off");
        compare(Q.scheduleLabel({mode: "sun", lat: 1, lon: 2}), "Sunset to sunrise");
        compare(Q.scheduleLabel({mode: "fixed", dark_at: "20:00", light_at: "07:30"}), "Dark 20:00–07:30");
        compare(Q.scheduleLabel({mode: "fixed", dark_at: "20:00:00", light_at: "07:30:00"}), "Dark 20:00–07:30");
        compare(Q.scheduleLabel({mode: "weird"}), "Unknown");
        compare(Q.scheduleLabel(null), "Unknown");
        compare(Q.nextSchedule({mode: "off"}, {lat: 51.5, lon: -0.1}), {mode: "sun", lat: 51.5, lon: -0.1});
        compare(Q.nextSchedule({mode: "off"}, null), null);
        compare(Q.nextSchedule({mode: "sun", lat: 1, lon: 2}, null), {mode: "off"});
        compare(Q.nextSchedule({mode: "fixed", dark_at: "20:00", light_at: "07:00"}, {lat: 1, lon: 2}), {mode: "off"});
        compare(Q.LAYOUTS, ["dwindle", "master", "scrolling"]);
    }

    function test_battery_and_profile_labels() {
        compare(Q.batteryLabel(82, false, 0), "82%");
        compare(Q.batteryLabel(82, false, 4800), "82% · 1 h 20 min");
        compare(Q.batteryLabel(82, false, 2700), "82% · 45 min");
        compare(Q.batteryLabel(82, false, 3600), "82% · 1 h");
        compare(Q.batteryLabel(82, true, 4800), "Charging 82%");
        compare(Q.batteryLabel(82.4, false, -5), "82%");
        compare(Q.profileLabel(0), "Power saver");
        compare(Q.profileLabel(1), "Balanced");
        compare(Q.profileLabel(2), "Performance");
        compare(Q.profileLabel(9), "Balanced");
    }
}
