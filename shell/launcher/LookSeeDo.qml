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
import "lookseedo"

// Super+Space search box: a top hit, then results grouped by type: apps, a typed web address,
// open windows, Settings, files by type (folders, documents, images, …) and by their text, the
// calculator, Run and other providers. Apps and windows are matched here; the bridge sends the
// rest. Before anything is typed it shows the apps opened lately (Recents) and five pinned
// squares the person fills; both go away as soon as there is text. A `>` command drops the
// default terminal down from the search pill; Enter opens that terminal docked beneath the pill,
// the drop-down's size, with the command running in it. The terminal belongs to look=see>do_:
// clicking it keeps look=see>do_ open, the next `>` command opens a new tab in it (kyko or kitty; other
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
    onDockAddressChanged: Surfaces.dockedTerminal = dockAddress
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
    readonly property var appList: L.appEntries(DesktopEntries.applications.values, StaleApps.hidden)

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

    SearchState {
        id: searchState
        root: root
        actionMenu: body.appActionMenu
        activateClose: activateClose
        copied: copied
        deadline: deadline
        debounce: debounce
        field: field
        termWatch: termWatch
    }
    function wipe() { searchState.wipe(); }

    function edited(text) { searchState.edited(text); }

    function runSearch() { return searchState.runSearch(); }

    // The open windows, read when a search runs (not kept up to date while look=see>do_ is shut).
    function windowList() { return searchState.windowList(); }

    // Nothing typed: the apps opened lately, in one group.
    function homeSections() { return searchState.homeSections(); }

    // The picker for square `pinSlot`: matching apps (or the first few by name), and a row that
    // empties the square when it holds an app.
    function pickSections() { return searchState.pickSections(); }

    function rebuild(reset) { searchState.rebuild(reset); }

    // Square `slot` of the pinned apps: open the picker for it.
    function startPick(slot) { searchState.startPick(slot); }

    function endPick() { searchState.endPick(); }

    function chooseForPin(item) { searchState.chooseForPin(item); }

    function onResults(data) { searchState.onResults(data); }

    function selectedSection() { return searchState.selectedSection(); }

    Activation {
        id: activation
        root: root
        activateClose: activateClose
        copied: copied
        debounce: debounce
        termWatch: termWatch
    }
    // Enter. If the rows are stale (typed within the debounce), search now and act on the fresh
    // top hit once the bridge replies: it emits the built-in calc/run rows before the reply, so
    // they're in place by then, and the bridge's run state is this generation's.
    function activate(alt) { activation.activate(alt); }

    // A click acts on the row the user clicked, so it's only honoured when the rows are current;
    // otherwise it just brings the rows up to date.
    function activateClicked(group, index) { activation.activateClicked(group, index); }

    function activateSelected(alt) { activation.activateSelected(alt); }

    // A pinned square (or Alt+N): open that app like a result row does.
    function openPinned(slot) { activation.openPinned(slot); }

    // Enter on a `>` command. With a terminal docked that takes tabs, a new tab in it; else the
    // default terminal opens docked beneath the pill (a docked one that can't take tabs is
    // minimised to the Open windows island first). look=see>do_ stays either way.
    function openInTerminal(item) { activation.openInTerminal(item); }

    function openNew(item) { activation.openNew(item); }

    TerminalDock {
        id: terminalDock
        root: root
    }
    function addressOf(t) { return terminalDock.addressOf(t); }

    // Where the docked terminal goes, in layout coordinates: under the pill and its strip, as
    // wide as look=see>do_. `scale` < 1 gives the same box shrunk about its centre, which is
    // where a tucked-away terminal waits so that it grows in when it comes back.
    function dockRect(scale) { return terminalDock.dockRect(scale); }

    // Whether the docked terminal's window is still there; forgets it once it's gone.
    function dockAlive() { return terminalDock.dockAlive(); }
    function forgetDock() { terminalDock.forgetDock(); }
    // The workspace shown on look=see>do_'s screen.
    function activeWsId() { return terminalDock.activeWsId(); }
    function placeDock(scale) { return terminalDock.placeDock(scale); }
    // look=see>do_ closed: the terminal goes to a special workspace of its own (it isn't in the
    // Open windows island while away), shrunk a little so it grows back in.
    function hideDock() { return terminalDock.hideDock(); }
    // look=see>do_ opened again: the terminal comes back under the pill on the workspace shown.
    function showDock() { return terminalDock.showDock(); }
    // A docked terminal that can't take a tab: minimised to the Open windows island, where it
    // can still be found, so a new one can dock.
    function releaseDock() { terminalDock.releaseDock(); }
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
                        root.placeDock(1).catch(function (e) { console.warn("uq[LAUNCH-003] dock the terminal: " + e); });
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
    function focusOnTerminal() { return terminalDock.focusOnTerminal(); }
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
    function pointerAt(p) { terminalDock.pointerAt(p); }

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

        ResultsPanel {
            id: body
            field: field
            pill: pill
            root: root
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
