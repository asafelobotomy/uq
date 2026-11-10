// Scrolls the nearest enclosing Flickable (a ScrollView's contentItem) just enough that `item` is
// fully in view, leaving it alone when it already is. Used by SettingRow before it flashes.
import QtQml

QtObject {
    function flickableOf(item) {
        for (let p = item ? item.parent : null; p; p = p.parent) {
            if (p.contentY !== undefined && p.contentHeight !== undefined && p.contentItem !== undefined
                    && p.flick !== undefined)
                return p;
        }
        return null;
    }

    function reveal(item, margin) {
        const f = flickableOf(item);
        if (!f)
            return;
        const m = margin || 0;
        const top = item.mapToItem(f.contentItem, 0, 0).y;
        const bottom = top + item.height;
        let y = f.contentY;
        if (top - m < f.contentY)
            y = top - m;
        else if (bottom + m > f.contentY + f.height)
            y = bottom + m - f.height;
        const lo = f.originY;
        const hi = Math.max(lo, f.originY + f.contentHeight - f.height);
        y = Math.min(Math.max(y, lo), hi);
        if (y !== f.contentY)
            f.contentY = y;
    }
}
