pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Widgets
import "../core"
import "../logic/launcher.js" as L
import "../logic/rank.js" as Rank
import "../logic/commands.js" as Commands
import "../logic/quickui.js" as QuickUi
import "../logic/shellprefs.js" as ShellPrefs
import "../logic/failure.js" as F
import "../logic/lookhome.js" as H

// Super+Space search box: a top hit, then results grouped by type: apps, a typed web address,
// open windows, Settings, files by type (folders, documents, images, …) and by their text, the
// calculator, Run and other providers. Apps and windows are matched here; the bridge sends the
// rest. Before anything is typed it shows the apps opened lately (Recents) and five pinned
// squares the person fills; both go away as soon as there is text. A `>` command drops the
// default terminal down from the search pill; Enter opens that terminal docked beneath the pill,
// the drop-down's size, with the command running in it. The terminal belongs to look=see>do_:
// clicking it keeps look=see>do_ open, the next `>` command opens a new tab in it (kitty; other
// terminals get a new docked window), it is tucked away when look=see>do_ closes and comes back,
// growing in like a new window, when it opens again. Logic lives in launcher.js and rank.js; everything shown from apps or providers goes
// through UqText, and Enter only ever makes bridge calls (Launch / Activate / Dispatch).
PanelWindow {
    id: root

    readonly property bool shown: Surfaces.open === "lookseedo" && !Bridge.locked
    // The focused monitor at the moment the surface opened, so an open surface stays put when
    // the cursor (and Hyprland's focus) crosses to another output. Followed live only while hidden.
    property var openedOn: null
    readonly property var target: Surfaces.screen ? Surfaces.screen
        : (openedOn ? openedOn
            : (Outputs.primaryScreen(Quickshell.screens) || screenFor(Hyprland.focusedMonitor)))
    readonly property real dpr: screen ? screen.devicePixelRatio : 1
    readonly property int iconSize: 24
    readonly property int rowHeight: 40
    // The bridge's 300 ms provider deadline plus slack for the round trip.
    readonly property int providerDeadlineMs: 500
    // Why the last activation, or the last search, failed (failure.js); "" when nothing failed.
    property string error: ""
    property string searchError: ""

    // --- home (nothing typed): pinned squares and their editing ---
    // The square being filled from the picker, or -1. While it is set the pill searches apps to
    // pin, and Enter pins instead of launching.
    property int pinSlot: -1
    // Edit mode: a click on a filled square changes it instead of opening the app.
    property bool editPins: false

    // --- the terminal that drops down for a `>` command ---
    // The default terminal ({id, name, icon}) read from the bridge when this opens; null until
    // then, and the drop-down says "a terminal".
    property var defaultTerminal: null
    // "" | "sending" (Launch asked for) | "sent" (its window is on screen). Typing again clears it.
    property string termState: ""
    property var termBefore: ({})
    // The handle the Launch for a new docked terminal gave back, until its window shows.
    property string pendingTabs: ""
    property int termTries: 0
    readonly property int termHeight: 220
    readonly property int pillHeight: 44
    readonly property int sentHeight: 36
    readonly property string termName: defaultTerminal && defaultTerminal.name
        ? String(defaultTerminal.name) : "a terminal"
    readonly property string command: L.classify(query) === "run" ? query.trim().slice(1).trim() : ""
    readonly property bool runMode: pinSlot < 0 && command !== ""
    readonly property bool homeShown: pinSlot < 0 && query.trim() === "" && !dockStrip

    // --- the docked terminal: the one a `>` command opened, kept with look=see>do_ ---
    // Its window, the handle that opens a new tab in it ("" when the terminal can't take tabs)
    // and whether it is tucked away (look=see>do_ closed). wipe() leaves these alone.
    property string dockAddress: ""
    property string dockTabs: ""
    property bool dockHidden: false
    readonly property bool docked: shown && dockAddress !== "" && !dockHidden
    // With the terminal docked, an empty field or a `>` command shows a one-line strip and the
    // terminal right under it, instead of Recents and the drop-down.
    readonly property bool dockStrip: docked && pinSlot < 0 && (query.trim() === "" || runMode)
    // False once a click went to the docked terminal: look=see>do_ stays open without its focus
    // grab, and closes when focus goes to any other window. Clicking the pill takes it back.
    property bool grabbed: true
    readonly property var squares: H.squares(LauncherState.pins, appList)

    // --- search state (all cleared by wipe()) ---
    property string query: ""
    property string searched: ""        // the query behind the current generation
    // The shared Surfaces counter: the bridge ignores a Search older than the newest it has seen.
    readonly property int generation: Surfaces.generation
    property var providerResults: ({})
    // ListProfiles' rows, read when this opens, for the Commands group's profile switches.
    property var profiles: []
    property var apps: []
    property var sections: []
    property var selection: ({ group: -1, index: -1 })
    property bool gotExternal: false
    property bool deadlinePassed: false
    readonly property bool emojiHint: L.showEmojiHint(searched, gotExternal, deadlinePassed)
    readonly property var appList: L.appEntries(DesktopEntries.applications.values)

    visible: shown
    screen: target
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "uq-lookseedo"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
    exclusionMode: ExclusionMode.Ignore
    anchors.top: true
    margins.top: Math.round((screen ? screen.height : 1080) * 0.18)
    implicitWidth: Math.min(640, Math.round((screen ? screen.width : 1280) * 0.5))
    // The tallest the panel can get, fixed: the panel grows and shrinks inside it as rows arrive,
    // so it never outruns the compositor's resize (a clipped panel with no bottom edge). Only the
    // panel takes input; a click below it falls through to what's underneath and closes look=see>do_.
    readonly property int maxListHeight: Math.round((screen ? screen.height : 1080) * 0.55)
    implicitHeight: maxListHeight + 200
    mask: Region { item: panel }

    function screenFor(monitor) {
        if (!monitor)
            return null;
        var list = Quickshell.screens;
        for (var i = 0; i < list.length; i++)
            if (list[i].name === monitor.name)
                return list[i];
        return null;
    }

    function wipe() {
        actionMenu.close();
        error = "";
        searchError = "";
        activateClose.stop();
        debounce.stop();
        deadline.stop();
        copied.stop();
        termWatch.stop();
        termState = "";
        grabbed = true;
        pinSlot = -1;
        editPins = false;
        field.text = "";
        query = "";
        searched = "";
        providerResults = ({});
        apps = [];
        sections = [];
        selection = ({ group: -1, index: -1 });
        gotExternal = false;
        deadlinePassed = false;
        // A new generation makes any answer still in flight stale, so it can't land after a quick
        // reopen; the empty Search also makes the bridge forget the last `>` command.
        Bridge.call("Search", ["", Surfaces.nextGeneration()]).catch(function () {});
        if (shown)
            rebuild(true);
    }

    function edited(text) {
        query = text;
        // Typing again starts over: a terminal opened for the last command stays on screen, but
        // the drop-down is for the command being typed now.
        if (termState !== "") {
            termWatch.stop();
            termState = "";
        }
        debounce.restart();
    }

    function runSearch() {
        Surfaces.nextGeneration();
        providerResults = ({});
        gotExternal = false;
        deadlinePassed = false;
        var q = query.trim();
        searched = q;
        if (pinSlot >= 0) {
            // Choosing an app to pin: apps only, matched here; the bridge isn't asked.
            apps = q !== "" ? Rank.searchApps(q, appList, Settings.launchCounts) : [];
            rebuild(true);
            return null;
        }
        apps = q !== "" && L.classify(q) === "search"
            ? Rank.searchApps(q, appList, Settings.launchCounts) : [];
        var wins = q !== "" && L.classify(q) === "search"
            && ShellPrefs.providerEnabled(Settings.values.shell, "windows")
            ? Rank.searchWindows(q, windowList()) : [];
        var cmds = q !== "" && L.classify(q) === "search"
            && ShellPrefs.providerEnabled(Settings.values.shell, "commands")
            ? Commands.search(q, profiles) : [];
        if (wins.length > 0 || cmds.length > 0) {
            var pr = {};
            if (wins.length > 0)
                pr.windows = { generation: generation, provider: "windows", provider_name: "Windows", items: wins };
            if (cmds.length > 0)
                pr.commands = { generation: generation, provider: "commands", provider_name: "Commands", items: cmds };
            providerResults = pr;
        }
        rebuild(true);
        // Always sent, even when empty, so the bridge drops a stale run command.
        var reply = Bridge.call("Search", [q, generation]);
        reply.then(function () {
            root.searchError = "";
        }).catch(function (e) {
            var text = F.failureText("Search isn't working", e);
            if (text !== root.searchError)
                console.warn("uq: Search failed: " + e);
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
        var r = H.recentEntries(LauncherState.recents, appList);
        return r.length > 0 ? [{ provider: "apps", providerName: "Recent", top: false, items: r }] : [];
    }

    // The picker for square `pinSlot`: matching apps (or the first few by name), and a row that
    // empties the square when it holds an app.
    function pickSections() {
        var items = searched !== "" ? apps.slice(0, 8) : H.pickList(appList, 8);
        var out = [];
        if (items.length > 0)
            out.push({ provider: "apps", providerName: "Pin to square " + (pinSlot + 1), top: false,
                       pick: true, items: items });
        if (LauncherState.pins[pinSlot] !== "")
            out.push({ provider: "apps", providerName: "", top: false, pick: true,
                       items: [{ id: "", name: "Remove from the pinned squares", icon: "list-remove",
                                 comment: "", remove: true }] });
        return out;
    }

    function rebuild(reset) {
        if (pinSlot >= 0)
            sections = pickSections();
        else if (searched === "")
            sections = homeSections();
        else
            sections = L.sections(Rank.merge(apps, providerResults, generation,
                ShellPrefs.providerOrder(Settings.values.shell)));
        // Nothing is chosen on the home view, so Enter never opens a recent app by accident.
        selection = reset && pinSlot < 0 && searched === "" ? ({ group: -1, index: -1 })
            : L.clampSelection(reset ? null : selection, sections);
    }

    // Square `slot` of the pinned apps: open the picker for it.
    function startPick(slot) {
        pinSlot = slot;
        field.text = "";
        query = "";
        searched = "";
        apps = [];
        providerResults = ({});
        rebuild(true);
        Qt.callLater(field.forceActiveFocus);
    }

    function endPick() {
        pinSlot = -1;
        field.text = "";
        query = "";
        searched = "";
        apps = [];
        rebuild(true);
    }

    function chooseForPin(item) {
        LauncherState.setPin(pinSlot, item.remove === true ? "" : String(item.id));
        endPick();
    }

    function onResults(data) {
        if (!shown || !data || data.generation !== generation || typeof data.provider !== "string")
            return;
        var pr = providerResults;
        pr[data.provider] = data;
        providerResults = pr;
        if (data.source_dir !== "builtin")
            gotExternal = true;
        rebuild(false);
    }

    function selectedSection() {
        return selection.group >= 0 ? sections[selection.group] : null;
    }

    // Enter. If the rows are stale (typed within the debounce), search now and act on the fresh
    // top hit once the bridge replies: it emits the built-in calc/run rows before the reply, so
    // they're in place by then, and the bridge's run state is this generation's.
    function activate(alt) {
        if (Bridge.locked)
            return;
        if (pinSlot >= 0) {
            if (L.resultsStale(debounce.running, query, searched)) {
                debounce.stop();
                runSearch();
            }
            activateSelected(alt);
            return;
        }
        if (L.resultsStale(debounce.running, query, searched)) {
            debounce.stop();
            var gen;
            var reply = runSearch();
            gen = generation;
            reply.then(function () {
                if (root.shown && generation === gen && !L.resultsStale(debounce.running, query, searched))
                    root.activateSelected(alt);
            }).catch(function () {
                // runSearch's own handler has logged it and shows it (searchError).
            });
            return;
        }
        activateSelected(alt);
    }

    // A click acts on the row the user clicked, so it's only honoured when the rows are current;
    // otherwise it just brings the rows up to date.
    function activateClicked(group, index) {
        if (Bridge.locked)
            return;
        if (pinSlot < 0 && L.resultsStale(debounce.running, query, searched)) {
            debounce.stop();
            runSearch();
            return;
        }
        selection = { group: group, index: index };
        activateSelected();
    }

    function activateSelected(alt) {
        var sec = selectedSection();
        if (!sec || Bridge.locked)
            return;
        if (pinSlot >= 0) {
            chooseForPin(sec.items[selection.index]);
            return;
        }
        if (sec.provider === "run" && alt !== true && sec.items[selection.index]) {
            openInTerminal(sec.items[selection.index]);
            return;
        }
        var a = L.activation(sec.provider, sec.items[selection.index], alt === true);
        if (a === null)
            return;
        var item = sec.items[selection.index];
        var label = String(item.name || item.title || item.id || "");
        var what = a.kind === "launch" ? "Couldn't open " + label
            : a.kind === "copy" ? "Couldn't copy that" : "Couldn't do that";
        // A refusal keeps look=see>do_ open and says why; it closes once the call worked (or after
        // activateClose, if that takes long).
        var fail = function (e) {
            activateClose.stop();
            console.warn("uq: " + a.call + " (" + sec.provider + ") failed: " + e);
            if (root.shown)
                root.error = F.failureText(what, e);
        };
        error = "";
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
        var sq = squares[slot];
        if (!sq || !sq.entry || Bridge.locked)
            return;
        error = "";
        activateClose.restart();
        Bridge.call("Launch", [{ desktop_id: sq.id }]).then(function () {
            activateClose.stop();
            LauncherState.recordRecent(sq.id);
            if (root.shown)
                Surfaces.close();
        }).catch(function (e) {
            activateClose.stop();
            console.warn("uq: Launch (pinned " + sq.id + ") failed: " + e);
            if (root.shown)
                root.error = F.failureText("Couldn't open " + String(sq.entry.name || sq.id), e);
        });
    }

    // Enter on a `>` command. With a terminal docked that takes tabs, a new tab in it; else the
    // default terminal opens docked beneath the pill (a docked one that can't take tabs is
    // minimised to the Open windows island first). look=see>do_ stays either way.
    function openInTerminal(item) {
        if (Bridge.locked || termState === "sending")
            return;
        error = "";
        if (dockAlive() && dockTabs !== "") {
            termState = "sending";
            Bridge.call("Launch", [{ run_id: item.id, terminal: true, tab: dockTabs }]).then(function () {
                root.termState = "sent";
            }).catch(function (e) {
                console.warn("uq: a new tab in the docked terminal failed: " + e);
                root.termState = "";
                root.releaseDock();
                root.openNew(item);
            });
            return;
        }
        if (dockAlive())
            releaseDock();
        openNew(item);
    }

    function openNew(item) {
        var before = {};
        var vals = Hyprland.toplevels.values;
        for (var i = 0; i < vals.length; i++)
            before[addressOf(vals[i])] = true;
        termBefore = before;
        pendingTabs = "";
        var sc = screen;
        var r = dockRect(1);
        // The window rules take the monitor's own coordinates.
        var place = { x: r.x - (sc ? sc.x : 0), y: r.y - (sc ? sc.y : 0), w: r.w, h: r.h,
                      monitor: sc ? String(sc.name) : "" };
        termState = "sending";
        Bridge.call("Launch", [{ run_id: item.id, terminal: true, dock: true, place: place }]).then(function (v) {
            root.pendingTabs = v && typeof v.tabs === "string" ? v.tabs : "";
            root.termTries = 0;
            termWatch.restart();
        }).catch(function (e) {
            console.warn("uq: Launch (run in a terminal) failed: " + e);
            root.termState = "";
            if (root.shown)
                root.error = F.failureText("Couldn't open " + root.termName, e);
        });
    }

    function addressOf(t) {
        var a = String(t.address || "");
        return a !== "" && a.indexOf("0x") !== 0 ? "0x" + a : a;
    }

    // Where the docked terminal goes, in layout coordinates: under the pill and its strip, as
    // wide as look=see>do_. `scale` < 1 gives the same box shrunk about its centre, which is
    // where a tucked-away terminal waits so that it grows in when it comes back.
    function dockRect(scale) {
        var sc = screen;
        var w = Math.round(root.implicitWidth), h = termHeight;
        var x = (sc ? sc.x : 0) + Math.round(((sc ? sc.width : 1280) - w) / 2);
        var y = (sc ? sc.y : 0) + root.margins.top + pillHeight + Theme.gapIn + sentHeight
            + Theme.gapOut * 2 + Theme.gapIn;
        var s = scale > 0 ? scale : 1;
        return { x: Math.round(x + w * (1 - s) / 2), y: Math.round(y + h * (1 - s) / 2),
                 w: Math.round(w * s), h: Math.round(h * s) };
    }

    // Whether the docked terminal's window is still there; forgets it once it's gone.
    function dockAlive() {
        if (dockAddress === "")
            return false;
        var vals = Hyprland.toplevels.values;
        for (var i = 0; i < vals.length; i++)
            if (addressOf(vals[i]) === dockAddress)
                return true;
        forgetDock();
        return false;
    }
    function forgetDock() {
        dockAddress = "";
        dockTabs = "";
        dockHidden = false;
    }
    // The workspace shown on look=see>do_'s screen.
    function activeWsId() {
        var m = screen ? Hyprland.monitorFor(screen) : null;
        var ws = m && m.activeWorkspace ? m.activeWorkspace : Hyprland.focusedWorkspace;
        return ws ? ws.id : 1;
    }
    function placeDock(scale) {
        var r = dockRect(scale);
        return Bridge.call("Dispatch", ["dock", { address: dockAddress, x: r.x, y: r.y, w: r.w, h: r.h }]);
    }
    // look=see>do_ closed: the terminal goes to a special workspace of its own (it isn't in the
    // Open windows island while away), shrunk a little so it grows back in.
    function hideDock() {
        if (dockHidden || !dockAlive())
            return;
        dockHidden = true;
        Bridge.call("Dispatch", ["move", { address: dockAddress, workspace: "special:uq-lookseedo" }]).then(function () {
            return root.placeDock(0.87);
        }).catch(function (e) { console.warn("uq: tuck the terminal away: " + e); });
    }
    // look=see>do_ opened again: the terminal comes back under the pill on the workspace shown.
    function showDock() {
        if (!dockHidden || !dockAlive())
            return;
        dockHidden = false;
        Bridge.call("Dispatch", ["move", { address: dockAddress, workspace: activeWsId() }]).then(function () {
            return root.placeDock(1);
        }).catch(function (e) { console.warn("uq: bring the terminal back: " + e); });
    }
    // A docked terminal that can't take a tab: minimised to the Open windows island, where it
    // can still be found, so a new one can dock.
    function releaseDock() {
        if (dockAlive())
            Bridge.call("Dispatch", ["minimise", { address: dockAddress, from_ws: activeWsId() }])
                .catch(function (e) { console.warn("uq: minimise the terminal: " + e); });
        forgetDock();
    }
    function regrab() {
        grabbed = true;
        field.forceActiveFocus();
    }

    // Looks for the window the terminal opened: the first one that wasn't there before. It
    // becomes the docked terminal, placed (and made floating) again in case its window rules
    // didn't take.
    Timer {
        id: termWatch
        interval: 120
        repeat: true
        onTriggered: {
            root.termTries++;
            var vals = Hyprland.toplevels.values;
            for (var i = 0; i < vals.length; i++) {
                var a = root.addressOf(vals[i]);
                if (a !== "" && !root.termBefore[a]) {
                    stop();
                    root.dockAddress = a;
                    root.dockTabs = root.pendingTabs;
                    root.dockHidden = false;
                    root.termState = "sent";
                    if (root.shown)
                        root.placeDock(1).catch(function (e) { console.warn("uq: dock the terminal: " + e); });
                    else
                        root.hideDock();
                    return;
                }
            }
            if (root.termTries >= 40) {
                stop();
                root.termState = "";
                if (root.shown)
                    root.error = "The terminal didn't open";
            }
        }
    }

    // Focus on the docked terminal (or the one opening) keeps look=see>do_.
    function focusOnTerminal() {
        var t = Hyprland.activeToplevel;
        if (!t)
            return false;
        var a = addressOf(t);
        return a !== "" && ((dockAddress !== "" && a === dockAddress)
                            || (termState === "sending" && !termBefore[a]));
    }
    // A click outside look=see>do_ ends its focus grab. Hyprland doesn't hand that click's focus
    // on while the grab holds it, so where the pointer is says where it went: on the docked
    // terminal, look=see>do_ stays (without the grab) and the terminal is given the keyboard;
    // anywhere else, look=see>do_ closes.
    function grabCleared() {
        if (!shown)
            return;
        if (!docked) {
            Surfaces.close();
            return;
        }
        if (!cursorReader.running)
            cursorReader.running = true;
    }
    Process {
        id: cursorReader
        command: ["hyprctl", "cursorpos", "-j"]
        stdout: StdioCollector {
            onStreamFinished: {
                var p = null;
                try {
                    p = JSON.parse(text);
                } catch (e) {
                }
                if (!p)
                    console.debug("uq: lookseedo dock: hyprctl cursorpos said " + text);
                root.pointerAt(p && typeof p.x === "number" ? p : null);
            }
        }
    }
    function pointerAt(p) {
        if (!shown)
            return;
        var r = dockRect(1);
        if (p && docked && p.x >= r.x && p.x < r.x + r.w && p.y >= r.y && p.y < r.y + r.h) {
            grabbed = false;
            Bridge.call("Dispatch", ["focus_window", { address: dockAddress, keep_pointer: true }])
                .catch(function (e) { console.warn("uq: focus the terminal: " + e); });
        } else {
            console.debug("uq: lookseedo dock: grab cleared at " + JSON.stringify(p) + ", terminal at "
                          + JSON.stringify(r) + (docked ? "" : " (not docked)") + ", closing");
            Surfaces.close();
        }
    }

    Connections {
        target: Hyprland
        // Without the grab (the terminal was clicked), focus going to any other window closes
        // look=see>do_, which tucks the terminal away.
        function onActiveToplevelChanged() {
            if (!root.shown || root.grabbed)
                return;
            var t = Hyprland.activeToplevel;
            if (t && !root.focusOnTerminal()) {
                console.debug("uq: lookseedo dock: focus went to " + root.addressOf(t) + ", closing");
                Surfaces.close();
            }
        }
        function onRawEvent(event) {
            // ADDRESS (without 0x): the docked terminal was closed (its last tab ended).
            if (event.name === "closewindow" && root.dockAddress !== ""
                    && "0x" + String(event.data).split(",")[0] === root.dockAddress)
                root.forgetDock();
        }
    }

    Timer {
        id: activateClose
        interval: 1500
        onTriggered: if (root.shown) Surfaces.close()
    }

    function moveSelection(key) {
        selection = L.move(selection, sections, key);
    }

    function iconSource(icon) {
        var r = L.iconRef(icon);
        if (r.path !== undefined)
            return r.path;
        return Quickshell.iconPath(r.theme, true) || Quickshell.iconPath(L.DEFAULT_ICON, true);
    }

    onShownChanged: {
        // The main display when one is chosen (the emoji bind opens this window too), else the
        // focused screen.
        openedOn = shown ? (Outputs.primaryScreen(Quickshell.screens)
                            || screenFor(Hyprland.focusedMonitor)) : null;
        wipe();
        if (shown)
            showDock();
        else
            hideDock();
        if (shown) {
            Qt.callLater(field.forceActiveFocus);
            Bridge.call("ListProfiles", []).then(function (v) {
                root.profiles = QuickUi.profileList(v);
            }).catch(function () { root.profiles = []; });
            Bridge.call("DefaultApps", []).then(function (v) {
                root.defaultTerminal = v && v.terminal ? v.terminal : null;
            }).catch(function () { root.defaultTerminal = null; });
        }
    }

    Connections {
        target: Bridge
        function onSearchResults(data) { root.onResults(data); }
        function onLockedChanged() {
            if (Bridge.locked) {
                root.wipe();
                Surfaces.lock();
            }
        }
    }
    Connections {
        target: Quickshell
        // Close if our monitor was unplugged.
        function onScreensChanged() {
            if (!root.shown || !Surfaces.screen)
                return;
            var list = Quickshell.screens;
            for (var i = 0; i < list.length; i++)
                if (list[i] === Surfaces.screen)
                    return;
            Surfaces.close();
        }
    }
    Connections {
        target: Surfaces
        function onWipe(surface) {
            if (surface === "lookseedo")
                root.wipe();
        }
        // The emoji bind: start at ":" as if typed, so the search runs as usual.
        function onPrefill(surface, text) {
            if (surface !== "lookseedo")
                return;
            field.text = text;
            root.edited(field.text);
            Qt.callLater(field.forceActiveFocus);
        }
    }

    Timer { id: debounce; interval: 70; onTriggered: root.runSearch() }
    Timer { id: deadline; interval: root.providerDeadlineMs; onTriggered: root.deadlinePassed = true }
    Timer { id: copied; interval: 600 }

    HyprlandFocusGrab {
        id: grab
        windows: [root]
        active: root.visible && root.grabbed
        onCleared: root.grabCleared()
    }

    // The search pill and, under it, the card that drops from it: Recents and the pinned
    // squares, the results, or the terminal for a `>` command.
    Item {
        id: panel
        width: parent.width

        implicitHeight: pill.height + (body.visible ? Theme.gapIn + body.height : 0)

        Rectangle {
            id: pill
            width: parent.width
            height: root.pillHeight
            radius: height / 2
            color: Theme.surface
            border.color: Theme.border
            border.width: Theme.borderWidth

            IconImage {
                id: glass
                anchors.left: parent.left
                anchors.leftMargin: Theme.gapOut + Theme.gapIn
                anchors.verticalCenter: parent.verticalCenter
                implicitSize: 18
                source: root.pinSlot >= 0 ? root.iconSource("list-add") : root.iconSource("system-search")
                opacity: 0.8
            }

            UqInput {
                id: field
                flat: true
                anchors.left: glass.right
                anchors.leftMargin: Theme.gapIn
                anchors.right: parent.right
                anchors.rightMargin: Theme.gapOut
                anchors.verticalCenter: parent.verticalCenter
                implicitHeight: 32
                maximumLength: 256
                placeholder: root.pinSlot >= 0 ? "Choose an app for square " + (root.pinSlot + 1)
                    : "Search apps, files, settings, windows, or type a web address"
                Accessible.name: "look=see>do_ search"
                onEdited: function (text) { root.edited(text); }
                Keys.onPressed: function (event) {
                    var alt = (event.modifiers & Qt.AltModifier) !== 0;
                    if (event.key === Qt.Key_Escape) {
                        // Backing out of the picker comes before closing.
                        if (root.pinSlot >= 0)
                            root.endPick();
                        else
                            Surfaces.close();
                    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        root.activate((event.modifiers & Qt.ShiftModifier) !== 0);
                    } else if (event.key === Qt.Key_Down) {
                        root.moveSelection("down");
                    } else if (event.key === Qt.Key_Up) {
                        root.moveSelection("up");
                    } else if (event.key === Qt.Key_Backtab
                               || (event.key === Qt.Key_Tab && (event.modifiers & Qt.ShiftModifier))) {
                        root.moveSelection("backtab");
                    } else if (event.key === Qt.Key_Tab) {
                        root.moveSelection("tab");
                    } else if (alt && root.homeShown && event.key >= Qt.Key_1 && event.key <= Qt.Key_5) {
                        root.openPinned(event.key - Qt.Key_1);
                    } else {
                        return;
                    }
                    event.accepted = true;
                }
            }
        }

        Rectangle {
            id: body
            y: pill.height + Theme.gapIn
            width: parent.width
            height: col.implicitHeight + Theme.gapOut * 2
            visible: root.sections.length > 0 || root.homeShown || root.runMode || root.pinSlot >= 0
                     || root.termState !== "" || root.dockStrip || root.error !== "" || root.searchError !== ""
                     || root.emojiHint || copied.running
                     || (root.searched !== "" && root.deadlinePassed)
            radius: Theme.radius
            color: Theme.surface
            border.color: Theme.border
            border.width: Theme.borderWidth

            ColumnLayout {
                id: col
                anchors.fill: parent
                anchors.margins: Theme.gapOut
                spacing: Theme.gapIn

                Flickable {
                    id: flick
                    Layout.fillWidth: true
                    Layout.preferredHeight: Math.min(results.implicitHeight, root.maxListHeight)
                    // Recents and the picker show with an empty field; once there is text the
                    // list is the search's, and a `>` command has the terminal instead.
                    visible: root.sections.length > 0 && !root.runMode && !root.dockStrip
                             && !(root.query.trim() !== "" && root.searched === "" && root.pinSlot < 0)
                    contentHeight: results.implicitHeight
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds

                    function ensureVisible(item) {
                        var p = item.mapToItem(results, 0, 0);
                        if (p.y < contentY)
                            contentY = p.y;
                        else if (p.y + item.height > contentY + height)
                            contentY = p.y + item.height - height;
                    }

                    Column {
                        id: results
                        width: flick.width
                        spacing: 2

                        Repeater {
                            model: root.sections
                            delegate: Column {
                                id: sec
                                required property var modelData
                                required property int index
                                width: results.width
                                spacing: 2

                                UqText {
                                    width: parent.width
                                    visible: text !== ""
                                    leftPadding: Theme.gapIn
                                    topPadding: sec.index > 0 ? Theme.gapIn : 0
                                    text: sec.modelData.top ? "Top hit" : sec.modelData.providerName
                                    color: Theme.textMuted
                                    font.pixelSize: Theme.fontSize - 1
                                }

                                Repeater {
                                    model: sec.modelData.items
                                    delegate: Rectangle {
                                        id: row
                                        required property var modelData
                                        required property int index
                                        readonly property bool isApp: sec.modelData.provider === "apps"
                                        readonly property bool selected: root.selection.group === sec.index
                                            && root.selection.index === index
                                        width: results.width
                                        height: root.rowHeight
                                        radius: Theme.radius - 2
                                        color: selected ? Theme.overlay
                                            : (rowMouse.containsMouse ? Theme.surfaceAlt : "transparent")
                                        Accessible.role: Accessible.ListItem
                                        Accessible.name: isApp ? String(modelData.name || "")
                                            : String(modelData.title || "")
                                        onSelectedChanged: if (selected) flick.ensureVisible(row)

                                        RowLayout {
                                            anchors.fill: parent
                                            anchors.leftMargin: Theme.gapIn
                                            anchors.rightMargin: Theme.gapIn
                                            spacing: Theme.gapIn * 2

                                            IconImage {
                                                Layout.preferredWidth: root.iconSize
                                                Layout.preferredHeight: root.iconSize
                                                implicitSize: root.iconSize
                                                backer.sourceSize.width: root.iconSize * root.dpr
                                                backer.sourceSize.height: root.iconSize * root.dpr
                                                source: root.iconSource(row.modelData.icon)
                                            }
                                            ColumnLayout {
                                                Layout.fillWidth: true
                                                spacing: 0
                                                UqText {
                                                    Layout.fillWidth: true
                                                    text: String((row.isApp ? row.modelData.name
                                                                            : row.modelData.title) || "")
                                                }
                                                UqText {
                                                    Layout.fillWidth: true
                                                    visible: text !== ""
                                                    text: String((row.isApp ? row.modelData.comment
                                                                            : row.modelData.subtitle) || "")
                                                    color: Theme.textMuted
                                                    font.pixelSize: Theme.fontSize - 1
                                                }
                                            }
                                            // The top hit names where it came from; other rows sit
                                            // under their group's heading.
                                            UqText {
                                                visible: sec.modelData.top === true
                                                Layout.maximumWidth: root.width * 0.3
                                                text: String(sec.modelData.providerName || "")
                                                color: Theme.textMuted
                                                font.pixelSize: Theme.fontSize - 1
                                            }
                                        }
                                        MouseArea {
                                            id: rowMouse
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                                            onClicked: function (mouse) {
                                                if (mouse.button === Qt.RightButton) {
                                                    // App actions from other apps (Phase 4 §2.4.2).
                                                    if (row.isApp && sec.modelData.pick !== true && row.modelData.id)
                                                        actionMenu.openAt(row, mouse.x, mouse.y,
                                                                          String(row.modelData.id || ""));
                                                } else {
                                                    root.activateClicked(sec.index, row.index);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // The pinned squares: open an app, or fill an empty square.
                ColumnLayout {
                    id: pinned
                    Layout.fillWidth: true
                    spacing: Theme.gapIn
                    visible: root.homeShown

                    RowLayout {
                        Layout.fillWidth: true
                        UqText {
                            Layout.fillWidth: true
                            leftPadding: Theme.gapIn
                            text: root.editPins ? "Pinned: choose a square to change" : "Pinned"
                            color: Theme.textMuted
                            font.pixelSize: Theme.fontSize - 1
                        }
                        Rectangle {
                            id: editButton
                            objectName: "lookseedoEditPins"
                            implicitWidth: editLabel.implicitWidth + Theme.gapOut
                            implicitHeight: 22
                            radius: height / 2
                            color: editMouse.containsMouse ? Theme.surfaceAlt : "transparent"
                            Accessible.role: Accessible.Button
                            Accessible.name: root.editPins ? "Done editing pinned apps" : "Edit pinned apps"
                            UqText {
                                id: editLabel
                                anchors.centerIn: parent
                                text: root.editPins ? "Done" : "Edit"
                                color: Theme.accent
                                font.pixelSize: Theme.fontSize - 1
                            }
                            MouseArea {
                                id: editMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                onClicked: root.editPins = !root.editPins
                            }
                        }
                    }

                    Row {
                        id: squareRow
                        Layout.fillWidth: true
                        readonly property int side: 76
                        spacing: Math.max(Theme.gapIn, (width - 5 * side) / 4)
                        Repeater {
                            model: root.squares
                            delegate: Rectangle {
                                id: sq
                                required property var modelData
                                readonly property bool filled: modelData.entry !== null
                                objectName: "lookseedoSquare" + modelData.slot
                                width: squareRow.side
                                height: squareRow.side
                                radius: Theme.radius
                                color: sqMouse.containsMouse ? Theme.overlay : Theme.surfaceAlt
                                border.color: root.editPins ? Theme.accent : Theme.border
                                border.width: Theme.borderWidth - 1
                                Accessible.role: Accessible.Button
                                Accessible.name: filled ? String(modelData.entry.name)
                                    : "Empty square " + (modelData.slot + 1) + ": pin an app"

                                IconImage {
                                    visible: sq.filled
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    y: Theme.gapIn * 2
                                    implicitSize: 32
                                    backer.sourceSize.width: 32 * root.dpr
                                    backer.sourceSize.height: 32 * root.dpr
                                    source: sq.filled ? root.iconSource(sq.modelData.entry.icon) : ""
                                }
                                UqText {
                                    visible: sq.filled
                                    anchors.left: parent.left
                                    anchors.right: parent.right
                                    anchors.bottom: parent.bottom
                                    anchors.margins: Theme.gapIn
                                    horizontalAlignment: Text.AlignHCenter
                                    text: sq.filled ? String(sq.modelData.entry.name) : ""
                                    font.pixelSize: Theme.fontSize - 1
                                }
                                UqText {
                                    visible: !sq.filled
                                    anchors.centerIn: parent
                                    text: "+"
                                    color: Theme.textMuted
                                    font.pixelSize: 24
                                }
                                // Edit mode: a small mark on a filled square that it can be changed.
                                UqText {
                                    visible: sq.filled && root.editPins
                                    anchors.top: parent.top
                                    anchors.right: parent.right
                                    anchors.margins: Theme.gapIn
                                    text: "…"
                                    color: Theme.accent
                                    font.pixelSize: Theme.fontSize
                                }
                                MouseArea {
                                    id: sqMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                                    onClicked: function (mouse) {
                                        // Empty squares, edit mode and a right click all choose the
                                        // app for the square; a plain click opens the app.
                                        if (!sq.filled || root.editPins || mouse.button === Qt.RightButton)
                                            root.startPick(sq.modelData.slot);
                                        else
                                            root.openPinned(sq.modelData.slot);
                                    }
                                }
                            }
                        }
                    }
                }

                // The default terminal, dropped down from the pill with the command being typed.
                // Enter opens the real one in its place.
                Rectangle {
                    id: termDrop
                    objectName: "lookseedoTerminal"
                    Layout.fillWidth: true
                    readonly property bool open: root.runMode && root.termState !== "sent" && !root.docked
                    Layout.preferredHeight: open ? root.termHeight : 0
                    visible: Layout.preferredHeight > 0
                    clip: true
                    radius: Theme.radius - 2
                    color: Theme.bg
                    border.color: Theme.border
                    border.width: Theme.borderWidth - 1
                    Behavior on Layout.preferredHeight {
                        NumberAnimation { duration: Motion.base; easing.type: Motion.easing }
                    }

                    Rectangle {
                        id: termBar
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        height: 28
                        color: Theme.surfaceAlt
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: Theme.gapOut
                            anchors.rightMargin: Theme.gapOut
                            spacing: Theme.gapIn * 2
                            IconImage {
                                Layout.preferredWidth: 16
                                Layout.preferredHeight: 16
                                implicitSize: 16
                                source: root.defaultTerminal && root.defaultTerminal.icon
                                    ? root.iconSource(root.defaultTerminal.icon)
                                    : root.iconSource("utilities-terminal")
                            }
                            UqText {
                                Layout.fillWidth: true
                                text: root.termName
                                color: Theme.textMuted
                                font.pixelSize: Theme.fontSize - 1
                            }
                        }
                    }
                    UqText {
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: termBar.bottom
                        anchors.margins: Theme.gapOut
                        baseFamily: Theme.fontMono
                        wrapMode: Text.WrapAnywhere
                        maximumLineCount: 7
                        // The same "prompt, command, cursor" the real terminal opens with.
                        text: "$ " + root.command + "\u2588"
                        color: Theme.text
                    }
                    UqText {
                        anchors.right: parent.right
                        anchors.bottom: parent.bottom
                        anchors.margins: Theme.gapOut
                        visible: root.termState === "sending"
                        text: "Opening…"
                        color: Theme.textMuted
                        font.pixelSize: Theme.fontSize - 1
                    }
                }

                // The terminal is docked under the pill: what Enter does next, or what is running.
                Rectangle {
                    objectName: "lookseedoSent"
                    Layout.fillWidth: true
                    Layout.preferredHeight: root.sentHeight
                    visible: root.dockStrip || (root.termState === "sent" && !root.docked)
                    radius: Theme.radius - 2
                    color: Theme.surfaceAlt
                    UqText {
                        anchors.fill: parent
                        anchors.leftMargin: Theme.gapOut
                        anchors.rightMargin: Theme.gapOut
                        verticalAlignment: Text.AlignVCenter
                        elide: Text.ElideRight
                        text: L.dockText(root.termName, root.dockTabs !== "", root.termState, root.runMode)
                        color: Theme.accent
                    }
                }

                UqText {
                    Layout.fillWidth: true
                    Layout.leftMargin: Theme.gapIn
                    // Nothing found, once the providers have had their time.
                    visible: root.sections.length === 0 && root.searched !== "" && root.deadlinePassed
                             && !root.emojiHint && root.query.trim() === root.searched
                             && !root.runMode && root.pinSlot < 0
                    text: "No matches"
                    color: Theme.textMuted
                }
                UqText {
                    Layout.fillWidth: true
                    Layout.leftMargin: Theme.gapIn
                    visible: root.pinSlot >= 0 && root.searched !== "" && root.sections.length === 0
                    text: "No apps match"
                    color: Theme.textMuted
                }
                UqText {
                    Layout.fillWidth: true
                    visible: root.emojiHint
                    text: "No emoji provider installed"
                    color: Theme.textMuted
                }
                UqText {
                    Layout.fillWidth: true
                    visible: copied.running
                    text: "Copied"
                    color: Theme.success
                }
                UqText {
                    objectName: "lookseedoError"
                    Layout.fillWidth: true
                    visible: text !== ""
                    text: root.error !== "" ? root.error : root.searchError
                    color: Theme.error
                    wrapMode: Text.Wrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                }
                UqText {
                    objectName: "lookseedoHint"
                    Layout.fillWidth: true
                    Layout.topMargin: Theme.gapIn
                    visible: root.termState !== "sent" && !root.dockStrip
                    text: root.pinSlot >= 0 ? "Enter pins the app   Esc goes back"
                        : (root.runMode ? "Enter runs it in " + root.termName + "   Shift+Enter runs it in the background"
                        : (root.homeShown ? "Alt+1 to 5 open a pinned app   = maths   > terminal   : emoji"
                        : "= maths   > terminal   : emoji   Tab next group"))
                    color: Theme.textMuted
                    font.pixelSize: Theme.fontSize - 1
                }
            }

            // More rows below: the list fades out instead of ending on a row cut in half.
            Rectangle {
                x: col.x + flick.x
                y: col.y + flick.y + flick.height - height
                width: flick.width
                height: Math.min(28, flick.height)
                visible: flick.visible && flick.contentY + flick.height < flick.contentHeight - 1
                gradient: Gradient {
                    GradientStop { position: 0; color: Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, 0) }
                    GradientStop { position: 1; color: Theme.surface }
                }
            }

            AppActionMenu {
                id: actionMenu
                onDone: Surfaces.close()
                onOpenChanged: if (!open && root.shown) field.forceActiveFocus()
            }
        }
        // After the docked terminal was clicked, the first click back on look=see>do_ only
        // takes the focus grab (and the keyboard) back.
        MouseArea {
            anchors.fill: parent
            z: 10
            enabled: !root.grabbed
            visible: enabled
            onPressed: root.regrab()
        }
    }
}
