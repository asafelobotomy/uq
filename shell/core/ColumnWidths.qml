pragma Singleton
import QtQuick

// Making a scrolling layout column Full or Half (the Open windows tabs' buttons). The screen
// pages (shell/windows/ScreenPages.qml) take it over when they run: a column made Half can leave
// the rest of its screen empty, and the window that would show there is hidden before the width
// changes, not after. Without them, the bridge's column_width is called directly. Returns the
// call's promise.
QtObject {
    id: root

    // function (address, full) -> Promise, set by ScreenPages.
    property var handler: null

    function set(address, full) {
        if (typeof root.handler === "function")
            return root.handler(address, full);
        return Bridge.call("Dispatch", ["column_width", { address: address, width: full ? "full" : "half" }]);
    }
}
