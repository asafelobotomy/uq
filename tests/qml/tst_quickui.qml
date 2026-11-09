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
        compare(U.volumeSet(0, 0, true), null);
        // A drag that doesn't change the whole percent writes nothing to Pipewire.
        compare(U.volumeSet(0.5, 0.504, false), null);
        compare(U.volumeSet(0.5, 0.51, false), { volume: 0.51, unmute: false });
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

    // WifiNetwork-shaped objects -> wifiRows rows, each carrying `index` back into the input so
    // a tap reaches the right live object. Open security is the only unsecured kind passed in;
    // anything else (including Unknown) counts as secured.
    function test_wifi_list() {
        var OPEN = 10, PSK = 3, UNKNOWN = 11;
        var nets = [
            { name: "cafe", signalStrength: 0.4, connected: false, known: false, security: OPEN },
            { name: "home", signalStrength: 0.7, connected: true, known: true, security: PSK },
            { name: "neighbour", signalStrength: 0.9, connected: false, known: false, security: PSK },
            { name: "", signalStrength: 1, connected: false, known: false, security: OPEN },
            null,
            { name: "odd", signalStrength: 0.2, connected: false, known: false, security: UNKNOWN }
        ];
        var rows = U.wifiList(nets, OPEN);
        compare(rows.map(function (r) { return r.name; }), ["home", "neighbour", "cafe", "odd"]);
        compare(rows.map(function (r) { return r.index; }), [1, 2, 0, 5]);
        compare(rows.map(function (r) { return r.connectable; }), [false, false, true, false]);
        verify(rows[3].secure);
        compare(rows[1].signal, 90);
        compare(U.wifiList(null, OPEN), []);
        compare(U.wifiList([], OPEN), []);
    }

    // Duplicate SSIDs (two Wi-Fi devices) each map back to their own object.
    function test_wifi_list_duplicates() {
        var nets = [
            { name: "dup", signalStrength: 0.3, connected: false, known: true, security: 3 },
            { name: "dup", signalStrength: 0.3, connected: false, known: true, security: 3 }
        ];
        var idx = U.wifiList(nets, 10).map(function (r) { return r.index; }).sort();
        compare(idx, [0, 1]);
    }

    // Untrusted SSIDs pass through untouched (UqText renders them as plain text).
    function test_wifi_list_untrusted_name() {
        var name = "<b>x</b>\u0007" + new Array(300).join("A");
        var rows = U.wifiList([{ name: name, signalStrength: 0.5, connected: false, known: false, security: 10 }], 10);
        compare(rows[0].name, name);
    }

    // A tap: disconnect when connected, connect when connectable, else (secured and unknown)
    // only point at Settings: the shell has no password prompt.
    function test_wifi_action() {
        compare(U.wifiAction({ connected: true, connectable: false }), "disconnect");
        compare(U.wifiAction({ connected: false, connectable: true }), "connect");
        compare(U.wifiAction({ connected: false, connectable: false }), "setup");
        compare(U.wifiAction(null), "");
    }

    function test_wired_label() {
        compare(U.wiredLabel({ connected: true, hasLink: true, linkSpeed: 1000 }), "Wired · Connected · 1000 Mb/s");
        compare(U.wiredLabel({ connected: true, hasLink: true, linkSpeed: 0 }), "Wired · Connected");
        compare(U.wiredLabel({ connected: false, hasLink: true, linkSpeed: 1000 }), "Wired · Cable plugged in, not connected");
        compare(U.wiredLabel({ connected: false, hasLink: false, linkSpeed: 0 }), "Wired · Cable unplugged");
        compare(U.wiredLabel(null), "");
    }

    // Paired devices only, connected first, otherwise in the order given.
    function test_bt_devices() {
        var a = { paired: true, connected: false };
        var b = { paired: false, connected: true };
        var c = { paired: true, connected: true };
        var d = { paired: true, connected: false };
        compare(U.btDevices([a, b, null, c, d]), [c, a, d]);
        compare(U.btDevices(null), []);
    }

    // Untrusted labels: name, then deviceName, then address; returned unchanged.
    function test_bt_label() {
        compare(U.btLabel({ name: "Buds", deviceName: "WH-1000", address: "AA:BB" }), "Buds");
        compare(U.btLabel({ name: "", deviceName: "WH-1000", address: "AA:BB" }), "WH-1000");
        compare(U.btLabel({ name: "", deviceName: "", address: "AA:BB" }), "AA:BB");
        compare(U.btLabel({}), "Unknown device");
        compare(U.btLabel(null), "Unknown device");
        compare(U.btLabel({ name: "<i>x</i>\n" }), "<i>x</i>\n");
    }

    function test_bt_battery() {
        compare(U.btBattery({ batteryAvailable: true, battery: 0.804 }), "80%");
        compare(U.btBattery({ batteryAvailable: true, battery: 1.5 }), "100%");
        compare(U.btBattery({ batteryAvailable: false, battery: 0.5 }), "");
        compare(U.btBattery({ batteryAvailable: true, battery: NaN }), "");
        compare(U.btBattery(null), "");
    }

    // GetTheme replies [name, variant]; anything else -> null.
    function test_theme_reply() {
        compare(U.themeReply(["nord", "dark"]), { name: "nord", variant: "dark" });
        compare(U.themeReply(["nord", "light"]), { name: "nord", variant: "light" });
        compare(U.themeReply(["nord", "sepia"]), null);
        compare(U.themeReply(["", "dark"]), null);
        compare(U.themeReply(["nord"]), null);
        compare(U.themeReply({ name: "nord" }), null);
        compare(U.themeReply(null), null);
    }

    function test_other_variant() {
        compare(U.otherVariant("dark"), "light");
        compare(U.otherVariant("light"), "dark");
        compare(U.otherVariant(""), "dark");
    }

    // ListThemes replies [ids]; non-strings and empties dropped.
    function test_theme_list() {
        compare(U.themeList(["a", "", 3, null, "b"]), ["a", "b"]);
        compare(U.themeList("a"), []);
        compare(U.themeList(null), []);
    }

    // GetLayouts keys are "<id>" or "name:<name>" (SystemIsland's lookup); dwindle by default.
    function test_workspace_layout() {
        var m = { "3": "master", "name:web": "scrolling" };
        compare(U.workspaceLayout(m, { id: 3, name: "3" }), "master");
        compare(U.workspaceLayout(m, { id: 7, name: "web" }), "scrolling");
        compare(U.workspaceLayout(m, { id: 9, name: "9" }), "dwindle");
        compare(U.workspaceLayout(null, { id: 3, name: "3" }), "dwindle");
        compare(U.workspaceLayout(m, null), "");
        // Named and special workspaces have negative ids: their key is the selector, not the id.
        var n = { "name:web": "master", "special:term": "scrolling", "-1337": "master" };
        compare(U.workspaceLayout(n, { id: -1337, name: "web" }), "master");
        compare(U.workspaceLayout(n, { id: -98, name: "special:term" }), "scrolling");
        compare(U.workspaceLayout({ "-1337": "master" }, { id: -1337, name: "web" }), "dwindle");
    }

    // Mirrors uqctl's active_workspace_selector (crates/uqctl/src/main.rs): the id when > 0,
    // a special workspace's own name, otherwise "name:<name>".
    function test_workspace_selector() {
        compare(U.workspaceSelector({ id: 3, name: "3" }), "3");
        compare(U.workspaceSelector({ id: -1337, name: "web" }), "name:web");
        compare(U.workspaceSelector({ id: -98, name: "special:term" }), "special:term");
        compare(U.workspaceSelector({ id: -99, name: "special" }), "special");
        compare(U.workspaceSelector({ id: 0, name: "" }), "");
        compare(U.workspaceSelector(null), "");
    }
}
