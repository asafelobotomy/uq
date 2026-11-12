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
}
