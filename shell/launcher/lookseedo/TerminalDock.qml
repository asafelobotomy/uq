// The docked terminal: where it sits under the pill, tucking it away and bringing it back, and whether focus or a click went to it.
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Widgets
import "../../core"
import "../../logic/launcher.js" as L
import "../../logic/rank.js" as Rank
import "../../logic/commands.js" as Commands
import "../../logic/quickui.js" as QuickUi
import "../../logic/shellprefs.js" as ShellPrefs
import "../../logic/failure.js" as F
import "../../logic/lookhome.js" as H
import "."

QtObject {
    // Passed in from LookSeeDo.qml.
    property var root

    function addressOf(t) {
        var a = String(t.address || "");
        return a !== "" && a.indexOf("0x") !== 0 ? "0x" + a : a;
    }

    // Where the docked terminal goes, in layout coordinates: under the pill and its strip, as
    // wide as look=see>do_. `scale` < 1 gives the same box shrunk about its centre, which is
    // where a tucked-away terminal waits so that it grows in when it comes back.
    function dockRect(scale) {
        var sc = root.screen;
        var w = Math.round(root.implicitWidth), h = root.termHeight;
        var x = (sc ? sc.x : 0) + Math.round(((sc ? sc.width : 1280) - w) / 2);
        var y = (sc ? sc.y : 0) + root.margins.top + root.pillHeight + Theme.gapIn + root.sentHeight
            + Theme.gapOut * 2 + Theme.gapIn;
        var s = scale > 0 ? scale : 1;
        return { x: Math.round(x + w * (1 - s) / 2), y: Math.round(y + h * (1 - s) / 2),
                 w: Math.round(w * s), h: Math.round(h * s) };
    }

    // Whether the docked terminal's window is still there; forgets it once it's gone.
    function dockAlive() {
        if (root.dockAddress === "")
            return false;
        var vals = Hyprland.toplevels.values;
        for (var i = 0; i < vals.length; i++)
            if (root.addressOf(vals[i]) === root.dockAddress)
                return true;
        root.forgetDock();
        return false;
    }

    function forgetDock() {
        root.dockAddress = "";
        root.dockTabs = "";
        root.dockHidden = false;
    }

    // The workspace shown on look=see>do_'s screen.
    function activeWsId() {
        var m = root.screen ? Hyprland.monitorFor(root.screen) : null;
        var ws = m && m.activeWorkspace ? m.activeWorkspace : Hyprland.focusedWorkspace;
        return ws ? ws.id : 1;
    }

    function placeDock(scale) {
        var r = root.dockRect(scale);
        return Bridge.call("Dispatch", ["dock", { address: root.dockAddress, x: r.x, y: r.y, w: r.w, h: r.h }]);
    }

    // look=see>do_ closed: the terminal goes to a special workspace of its own (it isn't in the
    // Open windows island while away), shrunk a little so it grows back in.
    function hideDock() {
        if (root.dockHidden || !root.dockAlive())
            return;
        root.dockHidden = true;
        Bridge.call("Dispatch", ["move", { address: root.dockAddress, workspace: "special:uq-lookseedo" }]).then(function () {
            return root.placeDock(0.87);
        }).catch(function (e) { console.warn("uq[LAUNCH-003] tuck the terminal away: " + e); });
    }

    // look=see>do_ opened again: the terminal comes back under the pill on the workspace shown.
    function showDock() {
        if (!root.dockHidden || !root.dockAlive())
            return;
        root.dockHidden = false;
        Bridge.call("Dispatch", ["move", { address: root.dockAddress, workspace: root.activeWsId() }]).then(function () {
            return root.placeDock(1);
        }).catch(function (e) { console.warn("uq[LAUNCH-003] bring the terminal back: " + e); });
    }

    // A docked terminal that can't take a tab: minimised to the Open windows island, where it
    // can still be found, so a new one can dock.
    function releaseDock() {
        if (root.dockAlive())
            Bridge.call("Dispatch", ["minimise", { address: root.dockAddress, from_ws: root.activeWsId() }])
                .catch(function (e) { console.warn("uq[LAUNCH-003] minimise the terminal: " + e); });
        root.forgetDock();
    }

    // Focus on the docked terminal (or the one opening) keeps look=see>do_.
    function focusOnTerminal() {
        var t = Hyprland.activeToplevel;
        if (!t)
            return false;
        var a = root.addressOf(t);
        return a !== "" && ((root.dockAddress !== "" && a === root.dockAddress)
                            || (root.termState === "sending" && !root.termBefore[a]));
    }

    function pointerAt(p) {
        if (!root.shown)
            return;
        var r = root.dockRect(1);
        if (p && root.docked && p.x >= r.x && p.x < r.x + r.w && p.y >= r.y && p.y < r.y + r.h) {
            root.grabbed = false;
            Bridge.call("Dispatch", ["focus_window", { address: root.dockAddress, keep_pointer: true }])
                .catch(function (e) { console.warn("uq[LAUNCH-003] focus the terminal: " + e); });
        } else {
            console.debug("uq: lookseedo dock: grab cleared at " + JSON.stringify(p) + ", terminal at "
                          + JSON.stringify(r) + (root.docked ? "" : " (not root.docked)") + ", closing");
            Surfaces.close();
        }
    }
}
