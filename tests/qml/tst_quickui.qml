import QtQuick
import QtTest
import "../../shell/logic/quickui.js" as U

TestCase {
    name: "quickui"

    // Fixed argv, integer percent clamped to 0..100; anything non-numeric is refused.
    function test_brightness_set_args() {
        compare(U.brightnessSetArgs(40), ["brightnessctl", "-m", "-e4", "-n2", "set", "40%"]);
        compare(U.brightnessSetArgs(40.6), ["brightnessctl", "-m", "-e4", "-n2", "set", "41%"]);
        compare(U.brightnessSetArgs(150)[5], "100%");
        compare(U.brightnessSetArgs(-3)[5], "0%");
        compare(U.brightnessSetArgs(NaN), null);
        compare(U.brightnessSetArgs(undefined), null);
        compare(U.brightnessSetArgs("50; rm -rf ~"), null);
    }

    // Raising from 0 while muted unmutes; any other move leaves mute alone.
    function test_volume_set() {
        compare(U.volumeSet(0, 0.3, true), { volume: 0.3, unmute: true });
        compare(U.volumeSet(0.2, 0.3, true), { volume: 0.3, unmute: false });
        compare(U.volumeSet(0, 0, true), { volume: 0, unmute: false });
        compare(U.volumeSet(0, 0.3, false), { volume: 0.3, unmute: false });
        compare(U.volumeSet(0.5, 1.7, false), { volume: 1, unmute: false });
        compare(U.volumeSet(0.5, -1, false), { volume: 0, unmute: false });
        compare(U.volumeSet(NaN, 0.4, true), { volume: 0.4, unmute: true });
        compare(U.volumeSet(0.5, NaN, false), null);
    }

    // Hardware sinks only: no streams, no sources, and only nodes with audio.
    function test_sink_nodes() {
        var a = { id: 1, isSink: true, isStream: false, audio: {} };
        var stream = { id: 2, isSink: true, isStream: true, audio: {} };
        var src = { id: 3, isSink: false, isStream: false, audio: {} };
        var video = { id: 4, isSink: true, isStream: false, audio: null };
        var b = { id: 5, isSink: true, isStream: false, audio: {} };
        compare(U.sinkNodes([a, stream, null, src, video, b]), [a, b]);
        compare(U.sinkNodes(null), []);
        compare(U.sinkNodes(undefined), []);
    }

    // Untrusted labels pass through unchanged; description, then nickname, then name.
    function test_node_label() {
        compare(U.nodeLabel({ description: "Speakers", nickname: "spk", name: "alsa.x" }), "Speakers");
        compare(U.nodeLabel({ description: "", nickname: "spk", name: "alsa.x" }), "spk");
        compare(U.nodeLabel({ description: "", nickname: "", name: "alsa.x" }), "alsa.x");
        compare(U.nodeLabel({}), "Unknown device");
        compare(U.nodeLabel(null), "Unknown device");
        compare(U.nodeLabel({ description: "<b>x</b>\u0007" }), "<b>x</b>\u0007");
    }

    function test_profile_options() {
        compare(U.profileOptions(true), [0, 1, 2]);
        compare(U.profileOptions(false), [0, 1]);
    }

    // Battery row only for a present laptop battery.
    function test_battery_shown() {
        verify(U.batteryShown({ isLaptopBattery: true, isPresent: true }));
        verify(!U.batteryShown({ isLaptopBattery: true, isPresent: false }));
        verify(!U.batteryShown({ isLaptopBattery: false, isPresent: true }));
        verify(!U.batteryShown(null));
    }
}
