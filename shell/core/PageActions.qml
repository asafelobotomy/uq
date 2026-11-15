pragma Singleton
import QtQuick

// What the Open windows tabs ask of the screen pages (shell/windows/ScreenPages.qml), which take
// it over when they run:
//   - setWidth: a scrolling layout column made Full or Half. Made Half, it can leave the rest of
//     its screen empty, and the window that would show there is hidden before the width changes.
//     Without the pages, the bridge's column_width is called directly.
//   - a tab dragged off the bar: `dropHint` says where it is ({screen (name), address, x, y (in
//     the screen's own coordinates), drop (over the screen, where it can be dropped), holdX,
//     holdY (where in the tab it is held), width, height, title, icon}; null when no tab is), so
//     a see-through copy of the tab can ride under the pointer and the screen can show where the
//     window would go; drop() places it there. Without the pages, nothing happens.
QtObject {
    id: root

    // Set by ScreenPages: function (address, full) -> Promise; function (screen, address, x).
    property var widthHandler: null
    property var dropHandler: null
    property var dropHint: null

    function setWidth(address, full) {
        if (typeof root.widthHandler === "function")
            return root.widthHandler(address, full);
        return Bridge.call("Dispatch", ["column_width", { address: address, width: full ? "full" : "half" }]);
    }
    function drop(screen, address, x) {
        root.dropHint = null;
        if (typeof root.dropHandler === "function")
            root.dropHandler(screen, address, x);
    }
}
