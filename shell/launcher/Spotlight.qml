pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import Quickshell.Widgets
import "../core"
import "../logic/launcher.js" as L
import "../logic/rank.js" as Rank
import "../logic/shellprefs.js" as ShellPrefs

// Super+Space search box: a top hit, then results grouped by source. Logic lives in
// launcher.js and rank.js; everything shown from apps or providers goes through UqText, and
// Enter only ever makes bridge calls (Launch / Activate).
PanelWindow {
    id: root

    readonly property bool shown: Surfaces.open === "spotlight" && !Bridge.locked
    // The focused monitor at the moment the surface opened, so an open surface stays put when
    // the cursor (and Hyprland's focus) crosses to another output. Followed live only while hidden.
    property var openedOn: null
    readonly property var target: Surfaces.screen ? Surfaces.screen
        : (openedOn ? openedOn : screenFor(Hyprland.focusedMonitor))
    readonly property real dpr: screen ? screen.devicePixelRatio : 1
    readonly property int iconSize: 24
    readonly property int rowHeight: 40
    // The bridge's 300 ms provider deadline plus slack for the round trip.
    readonly property int providerDeadlineMs: 500

    // --- search state (all cleared by wipe()) ---
    property string query: ""
    property string searched: ""        // the query behind the current generation
    // The shared Surfaces counter: the bridge ignores a Search older than the newest it has seen.
    readonly property int generation: Surfaces.generation
    property var providerResults: ({})
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
    WlrLayershell.namespace: "uq-spotlight"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
    exclusionMode: ExclusionMode.Ignore
    anchors.top: true
    margins.top: Math.round((screen ? screen.height : 1080) * 0.18)
    implicitWidth: Math.min(640, Math.round((screen ? screen.width : 1280) * 0.5))
    implicitHeight: panel.implicitHeight

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
        debounce.stop();
        deadline.stop();
        copied.stop();
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
    }

    function edited(text) {
        query = text;
        debounce.restart();
    }

    function runSearch() {
        Surfaces.nextGeneration();
        providerResults = ({});
        gotExternal = false;
        deadlinePassed = false;
        var q = query.trim();
        searched = q;
        apps = q !== "" && L.classify(q) === "search"
            ? Rank.searchApps(q, appList, Settings.launchCounts) : [];
        rebuild(true);
        // Always sent, even when empty, so the bridge drops a stale run command.
        var reply = Bridge.call("Search", [q, generation]);
        reply.catch(function () {});
        if (q !== "")
            deadline.restart();
        else
            deadline.stop();
        return reply;
    }

    function rebuild(reset) {
        sections = L.sections(Rank.merge(apps, providerResults, generation,
            ShellPrefs.providerOrder(Settings.values.shell)));
        selection = L.clampSelection(reset ? null : selection, sections);
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
    function activate() {
        if (Bridge.locked)
            return;
        if (L.resultsStale(debounce.running, query, searched)) {
            debounce.stop();
            var gen;
            var reply = runSearch();
            gen = generation;
            reply.then(function () {
                if (root.shown && generation === gen && !L.resultsStale(debounce.running, query, searched))
                    root.activateSelected();
            }).catch(function () {});
            return;
        }
        activateSelected();
    }

    // A click acts on the row the user clicked, so it's only honoured when the rows are current;
    // otherwise it just brings the rows up to date.
    function activateClicked(group, index) {
        if (Bridge.locked)
            return;
        if (L.resultsStale(debounce.running, query, searched)) {
            debounce.stop();
            runSearch();
            return;
        }
        selection = { group: group, index: index };
        activateSelected();
    }

    function activateSelected() {
        var sec = selectedSection();
        if (!sec || Bridge.locked)
            return;
        var a = L.activation(sec.provider, sec.items[selection.index]);
        if (a === null)
            return;
        var h = Bridge.call(a.call, a.args);
        if (a.kind === "copy") {
            h.then(function (reply) {
                var v = L.copyValue(reply);
                if (v !== null && root.shown) {
                    Quickshell.clipboardText = v;
                    copied.restart();
                }
            }).catch(function () {});
        } else {
            h.catch(function () {});
        }
        if (a.close)
            Surfaces.close();
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
        openedOn = shown ? screenFor(Hyprland.focusedMonitor) : null;
        wipe();
        if (shown)
            Qt.callLater(field.forceActiveFocus);
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
            if (surface === "spotlight")
                root.wipe();
        }
        // The emoji bind: start at ":" as if typed, so the search runs as usual.
        function onPrefill(surface, text) {
            if (surface !== "spotlight")
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
        windows: [root]
        active: root.visible
        onCleared: Surfaces.close()
    }

    Rectangle {
        id: panel
        width: parent.width
        implicitHeight: col.implicitHeight + Theme.gapOut * 2
        radius: Theme.radius
        color: Theme.surface
        border.color: Theme.border
        border.width: Theme.borderWidth

        ColumnLayout {
            id: col
            anchors.fill: parent
            anchors.margins: Theme.gapOut
            spacing: Theme.gapIn

            UqInput {
                id: field
                Layout.fillWidth: true
                implicitHeight: 36
                maximumLength: 256
                placeholder: "Search apps, = maths, > command"
                Accessible.name: "Spotlight search"
                onEdited: function (text) { root.edited(text); }
                Keys.onPressed: function (event) {
                    if (event.key === Qt.Key_Escape) {
                        Surfaces.close();
                    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        root.activate();
                    } else if (event.key === Qt.Key_Down) {
                        root.moveSelection("down");
                    } else if (event.key === Qt.Key_Up) {
                        root.moveSelection("up");
                    } else if (event.key === Qt.Key_Backtab
                               || (event.key === Qt.Key_Tab && (event.modifiers & Qt.ShiftModifier))) {
                        root.moveSelection("backtab");
                    } else if (event.key === Qt.Key_Tab) {
                        root.moveSelection("tab");
                    } else {
                        return;
                    }
                    event.accepted = true;
                }
            }

            Flickable {
                id: flick
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(results.implicitHeight,
                                                 (root.screen ? root.screen.height : 1080) * 0.55)
                visible: root.sections.length > 0
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
                                        // Every provider row names its provider.
                                        UqText {
                                            visible: !row.isApp
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
                                                if (row.isApp)
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
                Layout.fillWidth: true
                text: "= maths   > command   : emoji   Tab next group"
                color: Theme.textMuted
                font.pixelSize: Theme.fontSize - 1
            }
        }

        AppActionMenu {
            id: actionMenu
            onDone: Surfaces.close()
            onOpenChanged: if (!open && root.shown) field.forceActiveFocus()
        }
    }
}
