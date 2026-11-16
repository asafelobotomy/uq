import QtQuick
import QtTest
import "../../shell/logic/tray.js" as T

TestCase {
    name: "tray"

    function test_nm_applet_is_always_hidden() {
        verify(T.hiddenInTray("nm-applet", { notifiBell: false }));
        verify(T.hiddenInTray("nm-applet", null));
    }
    function test_notifi_hidden_only_when_it_is_the_bell() {
        verify(T.hiddenInTray("org.notify.Panel", { notifiBell: true }));
        verify(!T.hiddenInTray("org.notify.Panel", { notifiBell: false }));
        verify(!T.hiddenInTray("org.notify.Panel", undefined));
    }
    function test_other_items_stay() {
        verify(!T.hiddenInTray("Claude_status_icon_1", { notifiBell: true }));
        verify(!T.hiddenInTray("", { notifiBell: true }));
    }
    function test_find() {
        var items = [{ id: "a" }, null, { id: "org.notify.Panel", title: "noti-fi" }];
        compare(T.find(items, "org.notify.Panel").title, "noti-fi");
        compare(T.find(items, "zzz"), null);
        compare(T.find(null, "a"), null);
    }
    function test_notifi_bell() {
        verify(T.notifiBell("notifi", { id: "org.notify.Panel" }));
        verify(!T.notifiBell("builtin", { id: "org.notify.Panel" }));
        verify(!T.notifiBell("notifi", null));
    }

    function test_shown_items() {
        var items = [
            { id: "steam", status: 1 },
            { id: "nm-applet", status: 1 },
            { id: "quiet", status: T.PASSIVE },
            { id: "org.notify.Panel", status: 2 },
            null,
            { id: "discord", status: 2 }
        ];
        compare(T.shownItems(items, { notifiBell: true }).map(function (i) { return i.id; }),
                ["steam", "discord"]);
        compare(T.shownItems(items, { notifiBell: false }).map(function (i) { return i.id; }),
                ["steam", "org.notify.Panel", "discord"]);
        compare(T.shownItems(null, null), []);
    }

    function test_fit() {
        // Cells of 26 with 5 between: 4 fit in 119 px.
        compare(T.fit(3, 119, 26, 5), { shown: 3, more: 0 });
        compare(T.fit(4, 119, 26, 5), { shown: 4, more: 0 });
        // One more than fits: the last cell becomes "+2".
        compare(T.fit(5, 119, 26, 5), { shown: 3, more: 2 });
        // Icon size: only the "+N" button.
        compare(T.fit(3, 26, 26, 5), { shown: 0, more: 3 });
        compare(T.fit(0, 0, 26, 5), { shown: 0, more: 0 });
    }
}
