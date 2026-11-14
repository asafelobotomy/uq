pragma Singleton
import QtQuick
import "../logic/barlayout.js" as L

// The bar's layout as uqd has it (`shell.bar`, from Settings), edit mode, and the edits the bar
// makes: each sends the whole new layout with SetBarLayout; uqd validates it, saves it, and every
// bar redraws from the SettingsChanged that follows. Pure QML: loads under qmltestrunner.
QtObject {
    id: root

    // Settings.values; tests set their own.
    property var values: Settings.values
    readonly property var layout: L.layout(values)
    readonly property var options: layout.options || ({})
    // The last refusal from uqd, for the bar to show; "" when the last edit went through.
    property string error: ""
    // The bar popup that is open (IslandPopup); opening another closes it.
    property var activePopup: null

    // Sizes: the Icon size is a square island as tall as the bar; the other sizes are multiples
    // of it (barlayout.js UNITS).
    readonly property int unit: 36
    readonly property var metrics: ({
        unit: unit, gap: Theme.gapIn, zoneGap: Theme.gapOut,
        sep: Theme.borderWidth + Theme.gapIn * 2
    })

    // --- edit mode ---
    // While on, islands can be dragged, resized and set up; their own clicks are off. Every
    // screen's bar shows it; the bar on `editScreen` (a screen name) holds the keyboard.
    property bool editing: false
    property string editScreen: ""
    // The island the keyboard acts on in edit mode, as "zone:index"; "" for none.
    property string selected: ""

    function startEdit(screenName) {
        if (Bridge.locked)
            return;
        editScreen = screenName || "";
        selected = "";
        editing = true;
        if (activePopup)
            activePopup.hide();
    }
    function stopEdit() {
        editing = false;
        selected = "";
        if (activePopup)
            activePopup.hide();
    }
    function toggleEdit(screenName) {
        if (editing)
            stopEdit();
        else
            startEdit(screenName);
    }

    property Connections lockWatch: Connections {
        target: Bridge
        function onLockedChanged() {
            if (Bridge.locked)
                root.stopEdit();
        }
    }

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
        var next = L.move(layout, zone, index, dir);
        if (next !== null && selected === L.key(zone, index)) {
            var at = L.find(next, layout[zone][index].id);
            // Separators share an id: follow by position instead.
            if (layout[zone][index].id === L.SEPARATOR || at === null)
                at = _followMove(zone, index, dir);
            if (at !== null)
                selected = L.key(at.zone, at.index);
        }
        return save(next);
    }
    function _followMove(zone, index, dir) {
        var zi = L.ZONES.indexOf(zone), to = index + dir;
        if (to >= 0 && to < layout[zone].length)
            return { zone: zone, index: to };
        var nz = L.ZONES[zi + dir];
        return nz === undefined ? null : { zone: nz, index: dir < 0 ? layout[nz].length : 0 };
    }

    // Drop the island at zone/index on drop point `pt` (barlayout.js dropPoints).
    function drop(zone, index, pt) {
        var next = L.dropEdit(layout, zone, index, pt);
        if (next !== null && pt !== null) {
            var to = pt.index - (pt.zone === zone && pt.index > index ? 1 : 0);
            selected = L.key(pt.zone, Math.min(to, next[pt.zone].length - 1));
        }
        return save(next);
    }

    function remove(zone, index) {
        if (selected === L.key(zone, index))
            selected = "";
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

    // `units` only for a Custom size (its width in Icon sizes).
    function setSize(zone, index, size, units) {
        return save(L.setSize(layout, zone, index, size, units));
    }

    // Change the text style of the island at zone/index (barlayout.js setText).
    function setText(zone, index, t) {
        return save(L.setText(layout, zone, index, t));
    }

    function setOptions(island, opts) {
        return save(L.withOptions(layout, island, opts));
    }

    // Back to a new user's bar; the islands' options stay.
    function reset() {
        selected = "";
        return save({
            start: [{ id: "system" }, { id: "work" }], center: [],
            end: [{ id: "clock" }, { id: "notif" }, { id: "tray" }, { id: "status" }],
            options: layout.options
        });
    }
}
