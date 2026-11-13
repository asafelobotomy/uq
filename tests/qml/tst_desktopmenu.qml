import QtQuick
import QtTest
import "../../shell/logic/desktopmenu.js" as D

TestCase {
    name: "desktopmenu"

    function ids(list) {
        return list.map(function (e) { return e.id; });
    }

    function test_top_entries() {
        var top = D.entries();
        compare(ids(top), ["workspace", "open", "appearance", "system"]);
        compare(top[0].label, "Create workspace");
        compare(top[0].detail, "+");
        compare(top[1].sub, "open");
        compare(top[2].sub, "appearance");
        compare(top[3].sub, "system");
    }

    // The default apps by their own names and icons; an unset one is disabled, "Not set".
    function test_open_entries() {
        var apps = {
            terminal: { id: "foot.desktop", name: "Foot", icon: "foot" },
            files: null,
            text: { id: "org.kde.kate.desktop", name: "", icon: "" }
        };
        var l = D.subEntries("open", apps, "dark", true);
        compare(ids(l), ["terminal", "files", "text", "settings"]);
        compare(l[0].label, "Foot");
        compare(l[0].icon, "foot");
        compare(l[0].detail, "Terminal");
        compare(l[0].action, "launch");
        compare(l[0].appId, "foot.desktop");
        verify(D.enabled(l[0]));
        compare(l[1].label, "File Manager");
        compare(l[1].detail, "Not set");
        verify(!D.enabled(l[1]));
        compare(l[1].action, undefined);
        // No name or icon: the slot's own.
        compare(l[2].label, "Notepad");
        compare(l[2].icon, "accessories-text-editor");
        compare(l[2].appId, "org.kde.kate.desktop");
        compare(l[2].detail, "");
        compare(l[3].label, "uQ Settings");
        compare(l[3].action, "settings");
        // No reply yet, or garbage: every app slot is disabled, Settings still works.
        var none = D.subEntries("open", null, "dark", true);
        compare(none.filter(D.enabled).map(function (e) { return e.id; }), ["settings"]);
        var bad = D.subEntries("open", { terminal: { id: 5 }, files: "x", text: { id: "" } }, "dark", true);
        compare(bad.filter(D.enabled).length, 1);
    }

    function test_appearance_entries() {
        var l = D.subEntries("appearance", null, "light", true);
        compare(ids(l), ["wallpaper", "light", "dark"]);
        compare(l[0].action, "wallpaper");
        verify(l[1].checked);
        verify(!l[2].checked);
        compare(l[2].variant, "dark");
        l = D.subEntries("appearance", null, "dark", true);
        verify(!l[1].checked);
        verify(l[2].checked);
        // uqd down: the modes can't be switched.
        l = D.subEntries("appearance", null, "dark", false);
        verify(D.enabled(l[0]));
        verify(!D.enabled(l[1]));
        verify(!D.enabled(l[2]));
    }

    function test_system_entries() {
        var l = D.subEntries("system", null, "dark", true);
        compare(ids(l), ["logout", "lock", "reboot", "poweroff"]);
        compare(l.map(function (e) { return e.label; }), ["Log Off", "Lock", "Restart", "Shutdown"]);
        compare(l.map(function (e) { return e.power; }), ["logout", "lock", "reboot", "poweroff"]);
        // Restart and Shutdown go on to the power menu's confirm step.
        compare(l[2].detail, "…");
        compare(l[3].detail, "…");
        compare(D.subEntries("bogus", null, "dark", true), []);
    }

    function test_step_skips_disabled_and_wraps() {
        var l = [{ id: "a" }, { id: "b", enabled: false }, { id: "c" }];
        compare(D.first(l), 0);
        compare(D.step(l, 0, 1), 2);
        compare(D.step(l, 2, 1), 0);
        compare(D.step(l, 0, -1), 2);
        compare(D.step(l, -1, -1), 2);
        compare(D.first([{ enabled: false }]), -1);
        compare(D.first([]), -1);
        compare(D.first(null), -1);
    }

    function test_place_flips_and_clamps() {
        // Room: right of and below the pointer.
        compare(D.place(100, 100, 200, 150, 1920, 1080, 12), { x: 100, y: 100 });
        // Near the right and bottom edges: flipped to the left and above.
        compare(D.place(1900, 1000, 200, 150, 1920, 1080, 12), { x: 1700, y: 850 });
        // Flipped but still off the left: kept at the margin.
        compare(D.place(150, 100, 200, 150, 300, 1080, 12), { x: 12, y: 100 });
        // Bigger than the screen: at the margin.
        compare(D.place(10, 10, 500, 2000, 400, 1080, 12), { x: 12, y: 12 });
    }

    function test_place_sub() {
        // Room on the right: overlapping the menu by 5.
        compare(D.placeSub(100, 200, 140, 180, 120, 1920, 1080, 12, 5), { x: 295, y: 140 });
        // No room on the right: on the left.
        compare(D.placeSub(1700, 200, 140, 180, 120, 1920, 1080, 12, 5), { x: 1525, y: 140 });
        // Low row: moved up to stay on screen.
        compare(D.placeSub(100, 200, 1000, 180, 120, 1920, 1080, 12, 5), { x: 295, y: 948 });
        // No room either side: clamped.
        compare(D.placeSub(20, 260, 0, 180, 120, 300, 1080, 12, 0), { x: 108, y: 12 });
    }

    function test_new_workspace_id() {
        compare(D.newWorkspaceId([]), 1);
        compare(D.newWorkspaceId([1, 2, 3]), 4);
        compare(D.newWorkspaceId([1, 3]), 2);
        // Special workspaces (negative ids) don't count.
        compare(D.newWorkspaceId([-98, 1]), 2);
        compare(D.newWorkspaceId(null), 1);
        compare(D.newWorkspaceId(["2", 1]), 3);
    }
}
