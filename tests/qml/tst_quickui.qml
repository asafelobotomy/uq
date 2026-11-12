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

    // The row's status text (Q2: open networks are shown as Open).
    function test_wifi_status() {
        compare(U.wifiStatus({ connected: false, connectable: true, secure: false }), "Open");
        compare(U.wifiStatus({ connected: true, connectable: true, secure: false }), "Open · Connected");
        compare(U.wifiStatus({ connected: true, connectable: true, secure: true }), "Connected");
        compare(U.wifiStatus({ connected: false, connectable: true, secure: true }), "");
        compare(U.wifiStatus({ connected: false, connectable: false, secure: true }), "Set up in Settings");
        compare(U.wifiStatus(null), "");
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

    // ListThemeInfo replies {themes: [{id, name, ...}]}: malformed gives [], a non-string name
    // falls back to the id, and a long name is cut to 24 characters.
    function test_theme_info_list() {
        compare(U.themeInfoList(null), []);
        compare(U.themeInfoList("x"), []);
        compare(U.themeInfoList({}), []);
        compare(U.themeInfoList({ themes: "a" }), []);
        compare(U.themeInfoList({ themes: [
            { id: "a", name: "Alpha" },
            { id: "", name: "No id" },
            { id: 3, name: "Number id" },
            { id: "b", name: 7 },
            { id: "c" },
            null,
            { id: "d", name: "x".repeat(40) }
        ] }), [
            { id: "a", name: "Alpha" },
            { id: "b", name: "b" },
            { id: "c", name: "c" },
            { id: "d", name: "x".repeat(24) }
        ]);
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

    // Without a rule the workspace has the default layout ([layout_options] default), not dwindle.
    function test_workspace_layout_uses_default_layout() {
        var m = { "3": "master" };
        compare(U.workspaceLayout(m, { id: 9, name: "9" }, "scrolling"), "scrolling");
        compare(U.workspaceLayout(m, { id: 3, name: "3" }, "scrolling"), "master");
        compare(U.workspaceLayout(null, { id: 3, name: "3" }, "master"), "master");
        compare(U.workspaceLayout(m, null, "master"), "");
        // Unset or junk: Hyprland's own default.
        compare(U.workspaceLayout(m, { id: 9, name: "9" }, undefined), "dwindle");
        compare(U.workspaceLayout(m, { id: 9, name: "9" }, ""), "dwindle");
        compare(U.workspaceLayout(m, { id: 9, name: "9" }, 3), "dwindle");
        compare(U.defaultLayout({ layout_options: { default: "master" } }), "master");
        compare(U.defaultLayout({ layout_options: {} }), "");
        compare(U.defaultLayout({}), "");
        compare(U.defaultLayout(null), "");
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

    function test_profile_list() {
        compare(U.profileList([
            { id: "gaming", name: "Gaming", active: true, keys: 3 },
            { id: "work", name: "Work", active: false }
        ]), [
            { id: "gaming", name: "Gaming", active: true },
            { id: "work", name: "Work", active: false }
        ]);
        // Only a literal true is active.
        compare(U.profileList([{ id: "a", name: "A", active: "true" }])[0].active, false);
        compare(U.profileList([{ id: "a", name: "A", active: 1 }])[0].active, false);
        compare(U.profileList([{ id: "a", name: "A" }])[0].active, false);
        // Rows without a non-empty string id and name are dropped.
        compare(U.profileList([
            { id: 7, name: "Seven" },
            { id: "", name: "Empty" },
            { id: "x", name: "" },
            { id: "y", name: null },
            { name: "No id" },
            null,
            "gaming",
            { id: "ok", name: "Ok" }
        ]), [{ id: "ok", name: "Ok", active: false }]);
        // Untrusted names pass through untouched (UqText shows them as plain text).
        compare(U.profileList([{ id: "h", name: "<b>x</b>" }])[0].name, "<b>x</b>");
        // At most 50 rows.
        var many = [];
        for (var i = 0; i < 80; i++)
            many.push({ id: "p" + i, name: "P" + i });
        var rows = U.profileList(many);
        compare(rows.length, 50);
        compare(rows[49].id, "p49");
        // Malformed replies.
        compare(U.profileList(null), []);
        compare(U.profileList(undefined), []);
        compare(U.profileList("[]"), []);
        compare(U.profileList({ id: "gaming", name: "Gaming" }), []);
    }

    function test_active_profile() {
        compare(U.activeProfile([
            { id: "work", name: "Work", active: false },
            { id: "gaming", name: "Gaming", active: true }
        ]), "gaming");
        compare(U.activeProfile([{ id: "work", name: "Work", active: false }]), "");
        compare(U.activeProfile([]), "");
        compare(U.activeProfile(null), "");
        compare(U.activeProfile([{ id: "x", name: "X", active: "true" }]), "");
    }

    function test_profile_report_text() {
        compare(U.profileReportText({ active: "gaming", kept: [], notices: [] }), "");
        compare(U.profileReportText({
            active: "",
            kept: [{ domain: "look", path: ["gaps_in"], label: "Inner gaps" },
                   { domain: "look", path: ["blur"], label: "Blur" }],
            notices: []
        }), "Kept your own value: Inner gaps, Blur");
        compare(U.profileReportText({
            active: "gaming",
            kept: [],
            notices: ["Couldn't change the power mode.", "Another notice."]
        }), "Couldn't change the power mode. Another notice.");
        compare(U.profileReportText({
            active: "",
            kept: [{ label: "Blur" }, { label: "" }, null, { label: 3 }],
            notices: ["Couldn't change the power mode.", 5, ""]
        }), "Kept your own value: Blur Couldn't change the power mode.");
        // Malformed reports.
        compare(U.profileReportText(null), "");
        compare(U.profileReportText(undefined), "");
        compare(U.profileReportText("gaming"), "");
        compare(U.profileReportText({ active: "gaming" }), "");
        compare(U.profileReportText({ active: "", kept: "Blur", notices: [] }), "");
        compare(U.profileReportText({ active: "", kept: [], notices: "x" }), "");
    }

    function test_same_profiles() {
        var a = [{ id: "gaming", name: "Gaming", active: true }, { id: "work", name: "Work", active: false }];
        var b = [{ id: "gaming", name: "Gaming", active: true }, { id: "work", name: "Work", active: false }];
        verify(U.sameProfiles(a, b));
        verify(U.sameProfiles([], []));
        verify(!U.sameProfiles(a, [b[0]]));
        verify(!U.sameProfiles(a, [b[1], b[0]]));
        verify(!U.sameProfiles(a, [{ id: "gaming", name: "Games", active: true }, b[1]]));
        verify(!U.sameProfiles(a, [{ id: "gaming", name: "Gaming", active: false }, b[1]]));
        verify(!U.sameProfiles(a, [{ id: "other", name: "Gaming", active: true }, b[1]]));
        verify(!U.sameProfiles(a, null));
        verify(!U.sameProfiles(undefined, []));
        verify(!U.sameProfiles([null], [null]));
    }

    function test_profile_error_text() {
        // uqd's own refusal is plain words: shown as it is.
        compare(U.profileErrorText("The theme nord isn't installed."), "The theme nord isn't installed.");
        // Transport errors never name uqd or the bridge.
        var reach = "Couldn't reach uQ's settings service. Is it running?";
        compare(U.profileErrorText("uqd is not running"), reach);
        compare(U.profileErrorText("uqd did not answer within 20 s"), reach);
        compare(U.profileErrorText("bridge not running"), reach);
        compare(U.profileErrorText("bridge exited"), reach);
        compare(U.profileErrorText("reset"), reach);
        compare(U.profileErrorText("bad profile report: expected value"), "Couldn't switch profiles.");
        compare(U.profileErrorText(""), "Couldn't switch profiles.");
        compare(U.profileErrorText(null), "Couldn't switch profiles.");
        compare(U.profileErrorText(undefined), "Couldn't switch profiles.");
        compare(U.profileErrorText(3), "Couldn't switch profiles.");
    }

    // Five tabs in a fixed order; Apps hides while no app tile has loaded.
    function test_shown_tabs() {
        compare(U.shownTabs({}).map(function (t) { return t.key; }),
            ["sound", "network", "display", "system", "apps"]);
        compare(U.shownTabs({ apps: true }).map(function (t) { return t.key; }),
            ["sound", "network", "display", "system"]);
        compare(U.shownTabs(null).length, 5);
    }

    // The selected tab stays while shown; a hidden one falls back to the first without being
    // forgotten by the caller.
    function test_current_tab() {
        var all = U.shownTabs({});
        var noApps = U.shownTabs({ apps: true });
        compare(U.currentTab(all, "display"), "display");
        compare(U.currentTab(all, "apps"), "apps");
        compare(U.currentTab(noApps, "apps"), "sound");
        compare(U.currentTab(all, "bogus"), "sound");
        compare(U.currentTab([], "sound"), "");
        compare(U.currentTab(null, "sound"), "");
    }

    // Arrow keys wrap; Home and End jump to the ends.
    function test_step_tab() {
        var t = U.shownTabs({ apps: true });
        compare(U.stepTab(t, "sound", 1), "network");
        compare(U.stepTab(t, "system", 1), "sound");
        compare(U.stepTab(t, "sound", -1), "system");
        compare(U.stepTab(t, "display", -1), "network");
        compare(U.stepTab(t, "display", "first"), "sound");
        compare(U.stepTab(t, "sound", "last"), "system");
        compare(U.stepTab(t, "bogus", 1), "network");
        compare(U.stepTab([], "sound", 1), "");
    }
}
