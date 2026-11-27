// What the tabs do to their windows: focus or bring back, dispatch, Full and Half, icons and pins.
import QtQuick
import QtQuick.Effects
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import "../../core"
import "../../services"
import "../../notif"
import "../islands"
import "../../logic/minimise.js" as Min
import "../../logic/work.js" as Work
import "../../logic/dock.js" as Dock
import "."

QtObject {
    // Passed in from WorkIsland.qml.
    property var root

    function toplevelFor(address) {
        var vals = Hyprland.toplevels.values;
        for (var i = 0; i < vals.length; i++)
            if (Work.normAddr(vals[i].address) === address)
                return vals[i];
        return null;
    }

    // Focus a window; a minimised one comes back to the workspace it left.
    function choose(w) {
        var kids = w.children || [];
        if (!w.minimised && kids.length > 0) {
            // A window with a dialog open: its page comes into view, and the dialog (the last
            // one opened) gets focus, as it is the one waiting for an answer.
            var dialog = kids[kids.length - 1];
            Bridge.call("Dispatch", ["focus_window", { address: w.address, keep_pointer: true }])
                .then(function () {
                    return Bridge.call("Dispatch", ["focus_window", { address: dialog, keep_pointer: true }]);
                }).catch(function (e) { console.warn("uq[WIN-001] focus a window's dialog: " + e); });
            return;
        }
        if (w.minimised) {
            var t = root.toplevelFor(w.address);
            var ws = t && t.lastIpcObject ? Min.wsFromTags(t.lastIpcObject.tags) : null;
            if (ws === null)
                ws = root.wsId > 0 ? root.wsId : 1;
            // At the end of the queue: after the last window on the workspace (the scrolling
            // layout puts it next to the focused one).
            var last = Work.lastLive(root.windows, ws, w.address);
            var args = { address: w.address, to_ws: ws };
            if (last !== null)
                args.after = last;
            Bridge.call("Dispatch", ["restore", args]);
            // Its dialogs come back with it, after it.
            kids.forEach(function (c) {
                Bridge.call("Dispatch", ["restore", { address: c, to_ws: ws }]).catch(function () {});
            });
        } else if (!w.focused) {
            // The pointer stays put; a window out of view (a scrolled-away column) is brought
            // into view by the focus itself.
            Bridge.call("Dispatch", ["focus_window", { address: w.address, keep_pointer: true }]);
        }
    }

    function act(w, name, args) {
        var d = Work.buildDispatch(name, { addr: w.address, ws: root.wsId }, args);
        if (d !== null)
            Bridge.call("Dispatch", d);
        // Minimised or moved to another workspace, its dialogs go with it.
        if (name === "minimise" || name === "move")
            (w.children || []).forEach(function (c) {
                var dc = Work.buildDispatch(name, { addr: c, ws: root.wsId }, args);
                if (dc !== null)
                    Bridge.call("Dispatch", dc);
            });
    }

    // A tiled window in the scrolling layout, sized by its column (not real fullscreen, 2).
    function inColumn(w) {
        return w.scrolling && !w.floating && w.fullscreen !== 2;
    }

    // Full: the window spans its screen. In the scrolling layout that is a maximized column
    // (Hyprland's own Full: fullscreen 1) or one as wide as the screen; elsewhere, maximized or
    // fullscreen.
    function isFull(w) {
        return root.inColumn(w) ? (w.fullscreen === 1 || w.full === true) : w.fullscreen > 0;
    }

    // Make the window Full (the screen's width) or Half (half of it). Outside the scrolling
    // layout, Full maximizes and Half puts a maximized window back.
    function setFull(w, full) {
        if (w.minimised)
            return;
        if (root.inColumn(w))
            PageActions.setWidth(w.address, full)
                .catch(function (e) { console.warn("uq[WIN-001] window width: " + e); });
        else if (full !== root.isFull(w))
            root.act(w, "fullscreen", { mode: "maximized" });
        // The scrolling layout resizes columns without an event.
        root.refresh();
    }

    function iconFor(cls) {
        if (cls === "")
            return Quickshell.iconPath("application-x-executable", true);
        var a = root.appOf(cls);
        var name = a.icon ? a.icon : cls.toLowerCase();
        return Quickshell.iconPath(name, true) || Quickshell.iconPath("application-x-executable", true);
    }

    function togglePin(w) {
        var pins = Dock.togglePin(Settings.values.pinned || [], root.appOf(w.cls).id, Settings.loaded);
        if (pins !== null)
            Bridge.call("SetSetting", ["pinned", pins]);
    }

    function isPinned(w) {
        return (Settings.values.pinned || []).indexOf(root.appOf(w.cls).id) >= 0;
    }
}
