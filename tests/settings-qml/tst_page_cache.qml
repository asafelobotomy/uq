import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "PageCache"
    visible: true
    when: windowShown
    width: 400
    height: 300

    Component {
        id: cacheComp
        PageCache {
            width: 400
            height: 300
            keep: 3
        }
    }

    // A page that counts how often it was asked to read again.
    Component {
        id: pageComp
        Item {
            property string name
            property int refreshes: 0
            property int loaded: 0
            function refresh() {
                refreshes += 1;
            }
            Component.onCompleted: loaded += 1
        }
    }

    Component {
        id: closedComp
        Item {
            property bool keepAlive: false
        }
    }

    property int made: 0

    function maker(name) {
        return parent => {
            tc.made += 1;
            return pageComp.createObject(parent, { name: name });
        };
    }

    function init() {
        tc.made = 0;
    }

    function test_new_page_fills_and_shows() {
        const cache = createTemporaryObject(cacheComp, tc);
        const a = cache.show("a", "a.qml", maker("a"));
        verify(a);
        compare(cache.currentItem, a);
        compare(cache.currentKey, "a");
        verify(a.visible);
        compare(a.width, 400);
        compare(a.height, 300);
        compare(a.refreshes, 0, "a new page reads on its own; no refresh");
        cache.width = 500;
        compare(a.width, 500);
    }

    function test_going_back_shows_the_kept_page_and_refreshes_it() {
        const cache = createTemporaryObject(cacheComp, tc);
        const a = cache.show("a", "a.qml", maker("a"));
        const b = cache.show("b", "b.qml", maker("b"));
        verify(!a.visible, "the page left is hidden");
        verify(b.visible);
        const again = cache.show("a", "a.qml", maker("a"));
        compare(again, a, "the same page, not a new one");
        compare(tc.made, 2);
        verify(a.visible);
        verify(!b.visible);
        compare(a.refreshes, 1, "shown again: it reads again in the background");
        compare(cache.keys, ["b", "a"]);
    }

    function test_oldest_page_is_closed_past_keep() {
        const cache = createTemporaryObject(cacheComp, tc);
        const a = cache.show("a", "a.qml", maker("a"));
        cache.show("b", "b.qml", maker("b"));
        cache.show("c", "c.qml", maker("c"));
        compare(cache.keys, ["a", "b", "c"]);
        cache.show("a", "a.qml", maker("a"));
        cache.show("d", "d.qml", maker("d"));
        compare(cache.keys, ["c", "a", "d"], "b was shown longest ago");
        compare(tc.made, 4);
        cache.show("b", "b.qml", maker("b"));
        compare(tc.made, 5, "b is made again");
        compare(cache.keys, ["a", "d", "b"]);
        verify(!a.visible);
    }

    function test_changed_file_makes_a_new_page() {
        const cache = createTemporaryObject(cacheComp, tc);
        const a = cache.show("a", "v1/Page.qml", maker("a"));
        cache.show("b", "b.qml", maker("b"));
        const a2 = cache.show("a", "v2/Page.qml", maker("a"));
        verify(a2 !== a);
        compare(a2.refreshes, 0);
        compare(tc.made, 3);
        compare(cache.keys, ["b", "a"]);
    }

    function test_page_that_says_keepAlive_false_is_closed_when_left() {
        const cache = createTemporaryObject(cacheComp, tc);
        cache.show("net", "net.qml", parent => closedComp.createObject(parent));
        cache.show("b", "b.qml", maker("b"));
        compare(cache.keys, ["b"]);
        cache.show("net", "net.qml", parent => {
            tc.made += 1;
            return closedComp.createObject(parent);
        });
        compare(tc.made, 2, "made again, not kept");
    }

    function test_keepAlive_argument_false_is_not_kept() {
        const cache = createTemporaryObject(cacheComp, tc);
        cache.show("broken", "x.qml", maker("broken"), false);
        cache.show("b", "b.qml", maker("b"));
        compare(cache.keys, ["b"]);
    }

    function test_retain_closes_pages_gone_from_the_list() {
        const cache = createTemporaryObject(cacheComp, tc);
        cache.show("crumbs", "/usr/share/uq/settings/pages/crumbs/Page.qml", maker("crumbs"));
        cache.show("b", "b.qml", maker("b"));
        verify(!cache.retain((key, tag) => key !== "crumbs"), "the page on show stays");
        compare(cache.keys, ["b"]);
        verify(cache.retain((key, tag) => false), "the page on show went");
        compare(cache.keys, []);
        compare(cache.currentItem, null);
    }

    function test_create_failing_keeps_the_page_on_show() {
        const cache = createTemporaryObject(cacheComp, tc);
        const a = cache.show("a", "a.qml", maker("a"));
        compare(cache.show("b", "b.qml", parent => null), null);
        compare(cache.currentItem, a);
        verify(a.visible);
    }
}
