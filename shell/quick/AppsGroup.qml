import QtQuick
import QtQuick.Layouts
import "../core"
import "../widgets"
import "../logic/widgets.js" as W

// Quick settings: the "Apps" group, tiles from other apps' widgets (Phase 4 spec §2.2 `quick`
// slot), the same size as the toggle rows. A tile may open a detail view under the tiles. The
// group is hidden while no tile has loaded.
ColumnLayout {
    id: root

    property var registry: Widgets
    property var bridge: Bridge
    // The slot whose detail view is open, or null.
    property var detailSlot: null
    readonly property int loadedCount: host.loadedCount

    readonly property bool shown: W.appsGroupVisible(host.loadedCount)

    visible: shown
    spacing: Theme.gapIn * 2

    function wipe() {
        detailSlot = null;
    }

    UqText {
        text: "Apps"
        color: Theme.textMuted
        font.bold: true
    }

    WidgetHost {
        id: host
        Layout.fillWidth: true
        kind: "quick"
        registry: root.registry
        bridge: root.bridge
        onPopupRequested: function (slot, open) {
            if (open && slot.popup)
                root.detailSlot = slot;
            else if (root.detailSlot === slot)
                root.detailSlot = null;
        }
    }

    Loader {
        id: detail
        Layout.fillWidth: true
        active: root.detailSlot !== null && root.detailSlot.loaded && root.detailSlot.popup !== null
        visible: active
        sourceComponent: root.detailSlot ? root.detailSlot.popup : null
        onLoaded: {
            if (item.ctx !== undefined)
                item.ctx = root.detailSlot.ctx;
        }
    }

    Connections {
        target: Bridge
        function onLockedChanged() {
            if (Bridge.locked)
                root.wipe();
        }
    }
}
