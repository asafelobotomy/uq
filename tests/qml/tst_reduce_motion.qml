import QtQuick
import QtTest
import "../../shell/core"

// ui.reduce_motion reaches the shell through Settings (GetSettings / SettingsChanged); Dim and
// Wallpaper bind their reduceMotion to Settings.reduceMotion.
TestCase {
    name: "reduce_motion"

    function init() {
        Bridge.reset();
        Bridge.writer = function (s) {};
        Bridge.processStarted();
    }

    function settings(v) {
        Bridge.feed(JSON.stringify({ v: 1, event: "SettingsChanged", data: v }));
    }

    function test_follows_settings() {
        settings({ pinned: [] });
        compare(Settings.reduceMotion, false, "absent means off");
        settings({ ui: { reduce_motion: true } });
        compare(Settings.reduceMotion, true);
        settings({ ui: { reduce_motion: false } });
        compare(Settings.reduceMotion, false);
        settings({ ui: { reduce_motion: "yes" } });
        compare(Settings.reduceMotion, false, "only a real true counts");
        settings({ ui: null });
        compare(Settings.reduceMotion, false);
    }
}
