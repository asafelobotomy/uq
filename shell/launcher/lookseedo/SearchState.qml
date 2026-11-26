// The search: clearing it, running a query, the open windows and the picker, and the rows each answer adds.
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
    property var actionMenu
    property var activateClose
    property var copied
    property var deadline
    property var debounce
    property var field
    property var termWatch

    function wipe() {
        actionMenu.close();
        root.error = "";
        root.searchError = "";
        activateClose.stop();
        debounce.stop();
        deadline.stop();
        copied.stop();
        termWatch.stop();
        root.termState = "";
        root.grabbed = true;
        root.pinSlot = -1;
        root.editPins = false;
        field.text = "";
        root.query = "";
        root.searched = "";
        root.providerResults = ({});
        root.apps = [];
        root.sections = [];
        root.selection = ({ group: -1, index: -1 });
        root.gotExternal = false;
        root.deadlinePassed = false;
        // A new generation makes any answer still in flight stale, so it can't land after a quick
        // reopen; the empty Search also makes the bridge forget the last `>` command.
        Bridge.call("Search", ["", Surfaces.nextGeneration()]).catch(function () {});
        if (root.shown)
            root.rebuild(true);
    }

    function edited(text) {
        root.query = text;
        // Typing again starts over: a terminal opened for the last command stays on screen, but
        // the drop-down is for the command being typed now.
        if (root.termState !== "") {
            termWatch.stop();
            root.termState = "";
        }
        debounce.restart();
    }

    function runSearch() {
        Surfaces.nextGeneration();
        root.providerResults = ({});
        root.gotExternal = false;
        root.deadlinePassed = false;
        var q = root.query.trim();
        root.searched = q;
        if (root.pinSlot >= 0) {
            // Choosing an app to pin: apps only, matched here; the bridge isn't asked.
            root.apps = q !== "" ? Rank.searchApps(q, root.appList, Settings.launchCounts) : [];
            root.rebuild(true);
            return null;
        }
        root.apps = q !== "" && L.classify(q) === "search"
            ? Rank.searchApps(q, root.appList, Settings.launchCounts) : [];
        var wins = q !== "" && L.classify(q) === "search"
            && ShellPrefs.providerEnabled(Settings.values.shell, "windows")
            ? Rank.searchWindows(q, root.windowList()) : [];
        var cmds = q !== "" && L.classify(q) === "search"
            && ShellPrefs.providerEnabled(Settings.values.shell, "commands")
            ? Commands.search(q, root.profiles) : [];
        if (wins.length > 0 || cmds.length > 0) {
            var pr = {};
            if (wins.length > 0)
                pr.windows = { generation: root.generation, provider: "windows", provider_name: "Windows", items: wins };
            if (cmds.length > 0)
                pr.commands = { generation: root.generation, provider: "commands", provider_name: "Commands", items: cmds };
            root.providerResults = pr;
        }
        root.rebuild(true);
        // Always sent, even when empty, so the bridge drops a stale run command.
        var reply = Bridge.call("Search", [q, root.generation]);
        reply.then(function () {
            root.searchError = "";
        }).catch(function (e) {
            var text = F.failureText("Search isn't working", e);
            if (text !== root.searchError)
                console.warn("uq[LAUNCH-002] Search failed: " + e);
            if (root.shown)
                root.searchError = text;
        });
        if (q !== "")
            deadline.restart();
        else
            deadline.stop();
        return reply;
    }

    // The open windows, read when a search runs (not kept up to date while look=see>do_ is shut).
    function windowList() {
        var out = [], vals = Hyprland.toplevels.values;
        var mon = Hyprland.focusedMonitor;
        var toWs = mon && mon.activeWorkspace ? mon.activeWorkspace.id : 1;
        for (var i = 0; i < vals.length; i++) {
            var t = vals[i], o = t.lastIpcObject || ({});
            var cls = o["class"] ? String(o["class"]) : "";
            var d = cls ? DesktopEntries.heuristicLookup(cls) : null;
            var a = String(t.address || "");
            if (a !== "" && a.indexOf("0x") !== 0)
                a = "0x" + a;
            var ws = t.workspace ? t.workspace.name : "";
            out.push({ address: a, title: t.title || "", cls: cls,
                       app: d && d.name ? String(d.name) : cls, icon: d && d.icon ? String(d.icon) : "",
                       minimised: ws === "special:minimized",
                       workspace: ws.indexOf("special:") === 0 ? "" : ws, toWs: toWs });
        }
        return out;
    }

    // Nothing typed: the apps opened lately, in one group.
    function homeSections() {
        var r = H.recentEntries(LauncherState.recents, root.appList);
        return r.length > 0 ? [{ provider: "apps", providerName: "Recent", top: false, items: r }] : [];
    }

    // The picker for square `pinSlot`: matching apps (or the first few by name), and a row that
    // empties the square when it holds an app.
    function pickSections() {
        var items = root.searched !== "" ? root.apps.slice(0, 8) : H.pickList(root.appList, 8);
        var out = [];
        if (items.length > 0)
            out.push({ provider: "apps", providerName: "Pin to square " + (root.pinSlot + 1), top: false,
                       pick: true, items: items });
        if (LauncherState.pins[root.pinSlot] !== "")
            out.push({ provider: "apps", providerName: "", top: false, pick: true,
                       items: [{ id: "", name: "Remove from the pinned root.squares", icon: "list-remove",
                                 comment: "", remove: true }] });
        return out;
    }

    function rebuild(reset) {
        if (root.pinSlot >= 0)
            root.sections = root.pickSections();
        else if (root.searched === "")
            root.sections = root.homeSections();
        else
            root.sections = L.sections(Rank.merge(root.apps, root.providerResults, root.generation,
                ShellPrefs.providerOrder(Settings.values.shell)));
        // Nothing is chosen on the home view, so Enter never opens a recent app by accident.
        root.selection = reset && root.pinSlot < 0 && root.searched === "" ? ({ group: -1, index: -1 })
            : L.clampSelection(reset ? null : root.selection, root.sections);
    }

    // Square `slot` of the pinned apps: open the picker for it.
    function startPick(slot) {
        root.pinSlot = slot;
        field.text = "";
        root.query = "";
        root.searched = "";
        root.apps = [];
        root.providerResults = ({});
        root.rebuild(true);
        Qt.callLater(field.forceActiveFocus);
    }

    function endPick() {
        root.pinSlot = -1;
        field.text = "";
        root.query = "";
        root.searched = "";
        root.apps = [];
        root.rebuild(true);
    }

    function chooseForPin(item) {
        LauncherState.setPin(root.pinSlot, item.remove === true ? "" : String(item.id));
        root.endPick();
    }

    function onResults(data) {
        if (!root.shown || !data || data.generation !== root.generation || typeof data.provider !== "string")
            return;
        var pr = root.providerResults;
        pr[data.provider] = data;
        root.providerResults = pr;
        if (data.source_dir !== "builtin")
            root.gotExternal = true;
        root.rebuild(false);
    }

    function selectedSection() {
        return root.selection.group >= 0 ? root.sections[root.selection.group] : null;
    }
}
