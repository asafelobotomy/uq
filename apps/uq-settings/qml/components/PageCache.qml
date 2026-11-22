// Where the pages are shown. The pages shown most recently stay alive, hidden, so going back to
// one shows what it last read at once instead of an empty page that loads from scratch. It then
// reads again in the background (its `refresh()`, when it has one), so whatever changed
// meanwhile (an app uninstalled, a setting changed elsewhere) replaces the old values a moment
// later: stale while it revalidates.
//
// A page that must not stay alive while it is hidden declares `property bool keepAlive: false`
// (Bluetooth and Network answer pairing and password questions while they are open). Beyond
// `keep` pages, the one shown longest ago is closed.
import QtQuick

Item {
    id: cache

    /// How many pages stay alive, the shown one included.
    property int keep: 8
    /// The page on show, or null.
    readonly property Item currentItem: current ? current.item : null
    /// The id of the page on show ("" for none).
    readonly property string currentKey: current ? current.key : ""
    /// The ids of the pages kept, the one shown longest ago first (for tests).
    readonly property var keys: entries.map(e => e.key)

    /// `{key, tag, item, keepAlive}`, the one shown longest ago first.
    property var entries: []
    property var current: null

    /// Show page `key`. A kept page with the same `tag` (the page's file, so an update to it
    /// loads the new one) is shown again and asked to `refresh()`. Otherwise `create(parent)`
    /// makes it; it returns the item, or null when it couldn't. `keepAlive` false closes the
    /// page as soon as another is shown (a page that failed to load, say). Returns the item.
    function show(key, tag, create, keepAlive) {
        let entry = cache.entries.find(e => e.key === key);
        if (entry && entry.tag !== tag) {
            cache.drop(entry);
            entry = null;
        }
        const reused = entry !== null && entry !== undefined;
        if (!reused) {
            const item = create(cache);
            if (!item)
                return null;
            item.x = 0;
            item.y = 0;
            item.width = Qt.binding(() => cache.width);
            item.height = Qt.binding(() => cache.height);
            const wants = keepAlive !== false && item.keepAlive !== false;
            entry = { key: key, tag: tag, item: item, keepAlive: wants };
        }
        const previous = cache.current;
        cache.entries = cache.entries.filter(e => e !== entry).concat([entry]);
        cache.current = entry;
        entry.item.visible = true;
        if (previous && previous !== entry) {
            previous.item.visible = false;
            if (!previous.keepAlive)
                cache.drop(previous);
        }
        cache.trim();
        if (reused && typeof entry.item.refresh === "function")
            entry.item.refresh();
        return entry.item;
    }

    /// Close every kept page for which `wanted(key, tag)` is false (pages gone from the list, or
    /// whose file changed). Returns true when the page on show was one of them.
    function retain(wanted) {
        let currentGone = false;
        for (const e of cache.entries.slice()) {
            if (wanted(e.key, e.tag))
                continue;
            if (e === cache.current)
                currentGone = true;
            cache.drop(e);
        }
        return currentGone;
    }

    /// Close every page, the one on show included.
    function clear() {
        for (const e of cache.entries.slice())
            cache.drop(e);
    }

    function drop(entry) {
        cache.entries = cache.entries.filter(e => e !== entry);
        if (cache.current === entry)
            cache.current = null;
        entry.item.visible = false;
        entry.item.destroy();
    }

    function trim() {
        while (cache.entries.length > Math.max(1, cache.keep)) {
            const oldest = cache.entries.find(e => e !== cache.current);
            if (!oldest)
                return;
            cache.drop(oldest);
        }
    }
}
