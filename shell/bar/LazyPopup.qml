import QtQuick

// An IslandPopup made the first time it is shown, with the same show/hide/open/anchorItem: an
// island doesn't hold a window until its popup is wanted, and loads where no window can be made
// (tests/shell/bar_loads.sh, offscreen). Made from IslandPopup.qml by URL, so the window type is
// only looked up then.
Item {
    id: lazy

    property var popup: null
    readonly property bool open: popup ? popup.open : false
    readonly property Item anchorItem: popup ? popup.anchorItem : null

    function show(item, component, context) {
        if (popup === null) {
            var c = Qt.createComponent(Qt.resolvedUrl("IslandPopup.qml"));
            if (c.status !== Component.Ready) {
                console.warn("uq: island popup: " + c.errorString());
                return;
            }
            popup = c.createObject(lazy);
        }
        popup.show(item, component, context);
    }

    function hide() {
        if (popup)
            popup.hide();
    }
}
