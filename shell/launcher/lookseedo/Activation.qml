// What Enter and a click do: run a result, open a pinned square, or send a `>` command to the terminal.
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
    property var activateClose
    property var copied
    property var debounce
    property var termWatch

    // Enter. If the rows are stale (typed within the debounce), search now and act on the fresh
    // top hit once the bridge replies: it emits the built-in calc/run rows before the reply, so
    // they're in place by then, and the bridge's run state is this generation's.
    function activate(alt) {
        if (Bridge.locked)
            return;
        if (root.pinSlot >= 0) {
            if (L.resultsStale(debounce.running, root.query, root.searched)) {
                debounce.stop();
                root.runSearch();
            }
            root.activateSelected(alt);
            return;
        }
        if (L.resultsStale(debounce.running, root.query, root.searched)) {
            debounce.stop();
            var gen;
            var reply = root.runSearch();
            gen = root.generation;
            reply.then(function () {
                if (root.shown && root.generation === gen && !L.resultsStale(debounce.running, root.query, root.searched))
                    root.activateSelected(alt);
            }).catch(function () {
                // runSearch's own handler has logged it and shows it (searchError).
            });
            return;
        }
        root.activateSelected(alt);
    }

    // A click acts on the row the user clicked, so it's only honoured when the rows are current;
    // otherwise it just brings the rows up to date.
    function activateClicked(group, index) {
        if (Bridge.locked)
            return;
        if (root.pinSlot < 0 && L.resultsStale(debounce.running, root.query, root.searched)) {
            debounce.stop();
            root.runSearch();
            return;
        }
        root.selection = { group: group, index: index };
        root.activateSelected();
    }

    function activateSelected(alt) {
        var sec = root.selectedSection();
        if (!sec || Bridge.locked)
            return;
        if (root.pinSlot >= 0) {
            root.chooseForPin(sec.items[root.selection.index]);
            return;
        }
        if (sec.provider === "run" && alt !== true && sec.items[root.selection.index]) {
            root.openInTerminal(sec.items[root.selection.index]);
            return;
        }
        var a = L.activation(sec.provider, sec.items[root.selection.index], alt === true);
        if (a === null)
            return;
        var item = sec.items[root.selection.index];
        var label = String(item.name || item.title || item.id || "");
        var what = a.kind === "launch" ? "Couldn't open " + label
            : a.kind === "copy" ? "Couldn't copy that" : "Couldn't do that";
        // A refusal keeps look=see>do_ open and says why; it closes once the call worked (or after
        // activateClose, if that takes long).
        var fail = function (e) {
            activateClose.stop();
            console.warn("uq[LAUNCH-002] " + a.call + " (" + sec.provider + ") failed: " + e);
            if (root.shown)
                root.error = F.failureText(what, e);
        };
        root.error = "";
        var h = Bridge.call(a.call, a.args);
        if (a.kind === "copy") {
            h.then(function (reply) {
                var v = L.copyValue(reply);
                if (v !== null && root.shown) {
                    Quickshell.clipboardText = v;
                    copied.restart();
                }
            }).catch(fail);
        } else if (a.close) {
            activateClose.restart();
            h.then(function () {
                activateClose.stop();
                if (sec.provider === "apps")
                    LauncherState.recordRecent(String(item.id));
                if (root.shown)
                    Surfaces.close();
            }).catch(fail);
        } else {
            h.catch(fail);
        }
    }

    // A pinned square (or Alt+N): open that app like a result row does.
    function openPinned(slot) {
        var sq = root.squares[slot];
        if (!sq || !sq.entry || Bridge.locked)
            return;
        root.error = "";
        activateClose.restart();
        Bridge.call("Launch", [{ desktop_id: sq.id }]).then(function () {
            activateClose.stop();
            LauncherState.recordRecent(sq.id);
            if (root.shown)
                Surfaces.close();
        }).catch(function (e) {
            activateClose.stop();
            console.warn("uq[LAUNCH-001] Launch (pinned " + sq.id + ") failed: " + e);
            if (root.shown)
                root.error = F.failureText("Couldn't open " + String(sq.entry.name || sq.id), e);
        });
    }

    // Enter on a `>` command. With a terminal docked that takes tabs, a new tab in it; else the
    // default terminal opens docked beneath the pill (a docked one that can't take tabs is
    // minimised to the Open windows island first). look=see>do_ stays either way.
    function openInTerminal(item) {
        if (Bridge.locked || root.termState === "sending")
            return;
        root.error = "";
        if (root.dockAlive() && root.dockTabs !== "") {
            root.termState = "sending";
            Bridge.call("Launch", [{ run_id: item.id, terminal: true, tab: root.dockTabs }]).then(function () {
                root.termState = "sent";
            }).catch(function (e) {
                console.warn("uq[LAUNCH-003] a new tab in the root.docked terminal failed: " + e);
                root.termState = "";
                root.releaseDock();
                root.openNew(item);
            });
            return;
        }
        if (root.dockAlive())
            root.releaseDock();
        root.openNew(item);
    }

    function openNew(item) {
        var before = {};
        var vals = Hyprland.toplevels.values;
        for (var i = 0; i < vals.length; i++)
            before[root.addressOf(vals[i])] = true;
        root.termBefore = before;
        root.pendingTabs = "";
        var sc = root.screen;
        var r = root.dockRect(1);
        // The window rules take the monitor's own coordinates.
        var place = { x: r.x - (sc ? sc.x : 0), y: r.y - (sc ? sc.y : 0), w: r.w, h: r.h,
                      monitor: sc ? String(sc.name) : "" };
        root.termState = "sending";
        Bridge.call("Launch", [{ run_id: item.id, terminal: true, dock: true, place: place }]).then(function (v) {
            root.pendingTabs = v && typeof v.tabs === "string" ? v.tabs : "";
            root.termTries = 0;
            termWatch.restart();
        }).catch(function (e) {
            console.warn("uq[LAUNCH-001] Launch (run in a terminal) failed: " + e);
            root.termState = "";
            if (root.shown)
                root.error = F.failureText("Couldn't open " + root.termName, e);
        });
    }
}
