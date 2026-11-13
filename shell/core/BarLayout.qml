pragma Singleton
import QtQuick
import "../logic/barlayout.js" as L

// The bar's layout as uqd has it (`shell.bar`, from Settings), and the edits the bar itself
// makes: each sends the whole new layout with SetBarLayout; uqd validates it, saves it, and
// every bar redraws from the SettingsChanged that follows. Pure QML: loads under qmltestrunner.
QtObject {
    id: root

    // Settings.values; tests set their own.
    property var values: Settings.values
    readonly property var layout: L.layout(values)
    readonly property var options: layout.options || ({})
    // The last refusal from uqd, for the bar menu to show; "" when the last edit went through.
    property string error: ""
    // The bar popup that is open (IslandPopup); opening another closes it.
    property var activePopup: null

    function placed(id) {
        return L.placed(layout, id);
    }

    function save(next) {
        if (next === null || next === undefined)
            return false;
        Bridge.call("SetBarLayout", [L.toSetting(next)]).then(function () {
            root.error = "";
        }).catch(function (e) {
            root.error = String(e);
            console.warn("uq: bar layout refused: " + e);
        });
        return true;
    }

    function move(zone, index, dir) {
        return save(L.move(layout, zone, index, dir));
    }

    function remove(zone, index) {
        return save(L.remove(layout, zone, index));
    }

    // Remove island `id` wherever it is.
    function removeId(id) {
        var at = L.find(layout, id);
        return at !== null && remove(at.zone, at.index);
    }

    function add(id, zone, index) {
        return save(L.add(layout, id, zone, index));
    }

    // Add island `id` just before island `near` (or at the end of the right zone).
    function addBefore(id, near) {
        var at = L.find(layout, near);
        return at !== null ? add(id, at.zone, at.index) : add(id, "end");
    }

    function setSize(zone, index, size) {
        return save(L.setSize(layout, zone, index, size));
    }

    function setOptions(island, opts) {
        return save(L.withOptions(layout, island, opts));
    }
}
