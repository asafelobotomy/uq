import QtQuick
import "../core"

// An island's settings in a popup under the island, in edit mode (a left click on an island, or
// Enter on the selected one). The settings themselves are IslandForm.
IslandPopup {
    id: pop

    // {zone, index, id, size}: size is what the bar draws now (it may be smaller than chosen).
    property alias target: form.target

    function showFor(slot, t) {
        form.setTarget(t);
        show(slot);
    }

    IslandForm {
        id: form
        anchorItem: pop.anchorItem
        onDismissed: pop.hide()
    }
}
