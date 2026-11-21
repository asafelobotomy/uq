pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import "../core"
import "../session"
import "../logic/clip.js" as C

// The clipboard history (uqlip): a panel centred on the focused monitor with a search box and
// what was copied, newest first, pinned ones on top. Opened by SUPER+SHIFT+V, the bar island and
// `qs ipc call uq clip`.
//   Type            filters the list
//   Up / Down       move (PageUp / PageDown jump)
//   Enter           pastes the highlighted entry into the window you came from (Ctrl+Shift+V in
//                   terminals) and closes; clicking a row does the same
//   Ctrl+Enter      only puts it back on the clipboard, without pasting
//   Ctrl+1 .. 9     pastes that row
//   Ctrl+P          pins or unpins it       Delete   forgets it
//   Ctrl+Shift+Delete   forgets everything not pinned (no longer in the hint line, still works)
//   Esc             closes
// Text is shown as plain text (UqText); images as thumbnails. The highlighted text also shows
// below the list (the first 1500 characters), so a long copy can be told apart from another.
PanelWindow {
    id: root

    readonly property bool shown: ClipState.open && !Bridge.locked
    property var openedOn: null
    readonly property var target: openedOn ? openedOn : screenFor(Hyprland.focusedMonitor)
    property string query: ""
    property int current: 0
    property double now: Date.now() / 1000
    // Searching asks uqlip, which looks at the whole of each text; without a query it is the list.
    readonly property var rows: query.trim() === "" ? ClipState.entries : ClipState.found
    readonly property var currentEntry: current >= 0 && current < rows.length ? rows[current] : null
    readonly property int rowHeight: Theme.fontSize * 2 + Theme.gapIn * 2
    readonly property int imageHeight: rowHeight * 2

    visible: shown
    screen: target
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "uq-clip"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
    exclusionMode: ExclusionMode.Ignore
    implicitWidth: Theme.fontSize * 38
    implicitHeight: Math.min(Theme.fontSize * 38, Math.max(Theme.fontSize * 14, header.implicitHeight + list.contentHeight + Theme.gapOut * 3 + hint.implicitHeight + (previewPane.visible ? Theme.fontSize * 5 + Theme.gapIn * 2 : 0)))

    function screenFor(monitor) {
        if (!monitor)
            return null;
        var list = Quickshell.screens;
        for (var i = 0; i < list.length; i++)
            if (list[i].name === monitor.name)
                return list[i];
        return null;
    }

    function activate(i) {
        if (i >= 0 && i < rows.length)
            ClipState.paste(rows[i].id);
    }
    function copyOnly(i) {
        if (i >= 0 && i < rows.length)
            ClipState.copy(rows[i].id);
    }
    function windowClass() {
        var t = Hyprland.activeToplevel;
        var o = t ? t.lastIpcObject : null;
        return o && typeof o.class === "string" ? o.class : "";
    }

    onShownChanged: {
        if (shown)
            ClipState.targetClass = windowClass();
        ClipState.search("");
        openedOn = shown ? screenFor(Hyprland.focusedMonitor) : null;
        query = "";
        search.text = "";
        current = 0;
        now = Date.now() / 1000;
        if (shown)
            Qt.callLater(function () { search.forceActiveFocus(); });
    }
    onCurrentEntryChanged: ClipState.loadFull(currentEntry && currentEntry.kind === "text" ? currentEntry.id : -1)
    onRowsChanged: current = C.move(current, 0, rows.length) < 0 ? 0 : C.move(current, 0, rows.length)

    Connections {
        target: Bridge
        function onLockedChanged() {
            if (Bridge.locked)
                ClipState.hide();
        }
    }

    HyprlandFocusGrab {
        windows: [root]
        active: root.visible
        onCleared: ClipState.hide()
    }

    UqCard {
        anchors.fill: parent
        shown: root.shown
        Accessible.role: Accessible.Pane
        Accessible.name: "Clipboard history"
        focus: true
        Keys.onEscapePressed: ClipState.hide()

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: Theme.gapOut
            spacing: Theme.gapIn * 2

            RowLayout {
                id: header
                Layout.fillWidth: true
                spacing: Theme.gapIn
                UqInput {
                    id: search
                    Layout.fillWidth: true
                    implicitHeight: Theme.fontSize * 2 + 6
                    placeholder: ClipState.paused ? "Paused: new copies aren't kept" : "Search what you copied"
                    Accessible.name: "Search the clipboard history"
                    onEdited: text => { root.query = text; root.current = 0; ClipState.search(text); }
                    onAccepted: root.activate(root.current)
                    onKeyed: event => {
                        var ctrl = (event.modifiers & Qt.ControlModifier) !== 0;
                        var shift = (event.modifiers & Qt.ShiftModifier) !== 0;
                        if (event.key === Qt.Key_Down) {
                            root.current = C.move(root.current, 1, root.rows.length);
                        } else if (event.key === Qt.Key_Up) {
                            root.current = C.move(root.current, -1, root.rows.length);
                        } else if (event.key === Qt.Key_PageDown) {
                            root.current = C.move(root.current, 8, root.rows.length);
                        } else if (event.key === Qt.Key_PageUp) {
                            root.current = C.move(root.current, -8, root.rows.length);
                        } else if (event.key === Qt.Key_Escape) {
                            ClipState.hide();
                        } else if (ctrl && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) {
                            root.copyOnly(root.current);
                        } else if (ctrl && !shift && C.indexForDigit(event.text) >= 0) {
                            root.activate(C.indexForDigit(event.text));
                        } else if (ctrl && event.key === Qt.Key_P && root.rows.length > 0) {
                            var e = root.rows[root.current];
                            ClipState.pin(e.id, !e.pinned);
                        } else if (event.key === Qt.Key_Delete && ctrl && shift) {
                            ClipState.clear();
                        } else if (event.key === Qt.Key_Delete && root.rows.length > 0) {
                            ClipState.remove(root.rows[root.current].id);
                        } else {
                            return;
                        }
                        event.accepted = true;
                    }
                }
                Rectangle {
                    id: pauseButton
                    implicitWidth: pauseLabel.implicitWidth + Theme.gapIn * 3
                    implicitHeight: search.implicitHeight
                    radius: Theme.radiusControl
                    color: pauseMouse.containsMouse ? Theme.surfaceAlt : "transparent"
                    Behavior on color { ColorAnimation { duration: Motion.fast } }
                    border.color: ClipState.paused ? Theme.warning : Theme.border
                    border.width: Theme.borderWidth
                    Accessible.role: Accessible.Button
                    Accessible.name: ClipState.paused ? "Resume recording" : "Pause recording"
                    Accessible.onPressAction: ClipState.setPaused(!ClipState.paused)
                    UqText {
                        id: pauseLabel
                        anchors.centerIn: parent
                        text: ClipState.paused ? "Resume" : "Pause"
                        color: ClipState.paused ? Theme.warning : Theme.textMuted
                    }
                    MouseArea {
                        id: pauseMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: ClipState.setPaused(!ClipState.paused)
                    }
                }
            }

            UqText {
                visible: root.rows.length === 0
                Layout.fillWidth: true
                Layout.fillHeight: true
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                color: Theme.textMuted
                wrapMode: Text.WordWrap
                text: !ClipState.connected ? "uqlip isn't running."
                    : (root.query !== "" ? "Nothing matches." : "Nothing copied yet.")
            }

            ListView {
                id: list
                visible: root.rows.length > 0
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 2
                model: root.rows
                currentIndex: root.current
                highlightFollowsCurrentItem: true
                boundsBehavior: Flickable.StopAtBounds
                delegate: Rectangle {
                    id: row
                    required property var modelData
                    required property int index
                    readonly property bool lit: row.index === root.current || rowMouse.containsMouse
                    width: ListView.view.width
                    height: modelData.kind === "image" ? root.imageHeight : root.rowHeight
                    radius: Theme.radiusControl
                    color: row.index === root.current ? Theme.surfaceAlt : "transparent"
                    Behavior on color { ColorAnimation { duration: Motion.fast } }
                    border.color: row.index === root.current ? Theme.accent : "transparent"
                    border.width: row.index === root.current ? 1 : 0
                    Accessible.role: Accessible.ListItem
                    Accessible.name: C.label(modelData, 200)

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: Theme.gapIn * 2
                        anchors.rightMargin: Theme.gapIn * 2
                        spacing: Theme.gapIn * 2

                        UqText {
                            visible: row.modelData.pinned
                            text: "📌"
                            Accessible.ignored: true
                        }
                        Image {
                            visible: row.modelData.kind === "image"
                            Layout.preferredWidth: root.imageHeight * 1.5
                            Layout.preferredHeight: root.imageHeight - Theme.gapIn * 2
                            source: row.modelData.path !== "" ? "file://" + row.modelData.path : ""
                            fillMode: Image.PreserveAspectFit
                            sourceSize.height: root.imageHeight
                            asynchronous: true
                            cache: false
                        }
                        UqText {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            maximumLineCount: 1
                            text: C.label(row.modelData, 200)
                        }
                        UqText {
                            visible: row.index < 9
                            text: String(row.index + 1)
                            color: Theme.textMuted
                            Accessible.ignored: true
                        }
                        UqText {
                            text: C.age(row.modelData.created, root.now)
                            color: Theme.textMuted
                        }
                    }
                    MouseArea {
                        id: rowMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onPositionChanged: root.current = row.index
                        onClicked: root.activate(row.index)
                    }
                }
            }

            UqText {
                id: previewPane
                visible: root.currentEntry !== null && root.currentEntry.kind === "text"
                    && ClipState.fullText !== "" && ClipState.fullText.length > root.currentEntry.preview.length - 1
                Layout.fillWidth: true
                Layout.preferredHeight: visible ? Theme.fontSize * 5 : 0
                wrapMode: Text.Wrap
                elide: Text.ElideRight
                maximumLineCount: 4
                color: Theme.textMuted
                text: ClipState.fullText
                Accessible.ignored: true
            }

            UqText {
                id: hint
                Layout.fillWidth: true
                color: Theme.textMuted
                text: "Enter paste   Ctrl+Enter copy   Ctrl+P pin   Del forget   Esc close"
                elide: Text.ElideRight
            }
        }
    }
}
