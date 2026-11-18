import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import "../core"
import "../logic/barlayout.js" as L
import "islands"

// The Bar editor: edit mode's dialog, which opens with it (and again from the edit panel's
// Settings button or S) on the screen editing started on. Left: the islands on the bar, in order,
// and every island type to add (or to jump to when it is on the bar already). Right: the chosen
// island's settings (IslandForm: size, text, what it shows) or "All islands": one size or text
// style for every island at once, and Reset. The screen below the bar is dimmed while it is open;
// the bar stays lit and live, so each change shows at once (BarLayout saves it at once). It holds
// the keyboard: Tab and Up/Down move, Enter acts, Esc closes it (edit mode goes on until Done).
PanelWindow {
    id: win

    // The Bar this editor belongs to.
    property var bar: null
    property bool active: false
    // "island": the chosen island's settings; "all": the settings for every island.
    property string page: "island"

    readonly property bool shown: active && BarLayout.editorOpen && !Bridge.locked
    // Under the bar, so the bar stays lit.
    readonly property int below: WindowLook.gapTop + BarLayout.unit + Theme.gapIn

    screen: bar ? bar.screen : null
    visible: shown || scrim.opacity > 0
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.namespace: "uq-bar-editor"
    WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
    exclusionMode: ExclusionMode.Ignore
    anchors { top: true; bottom: true; left: true; right: true }
    margins.top: below

    // Every island on the bar, in bar order.
    readonly property var order: L.order(BarLayout.layout, null).map(function (o) {
        var it = BarLayout.layout[o.zone][o.index];
        return { zone: o.zone, index: o.index, key: o.key, id: it.id };
    })
    readonly property var zoneNames: ({ start: "Left", center: "Centre", end: "Right" })

    // The chosen island, as IslandForm takes it; null when none is chosen.
    readonly property var selTarget: {
        for (var i = 0; i < order.length; i++)
            if (order[i].key === BarLayout.selected) {
                var o = order[i], slot = bar ? bar.slotFor(o.key) : null, it = BarLayout.layout[o.zone][o.index];
                return { zone: o.zone, index: o.index, id: o.id, size: slot ? slot.size : L.sizeOf(it), units: slot ? slot.units : L.unitsOf(it) };
            }
        return null;
    }
    readonly property var selSlot: selTarget && bar ? bar.slotFor(BarLayout.selected) : null

    function close() {
        BarLayout.editorOpen = false;
    }
    // Choose island `key`, showing its settings.
    function choose(key) {
        BarLayout.selected = key;
        page = "island";
    }
    // A type on the right: choose it when it is on the bar, else add it at the end.
    function pickType(id) {
        var at = L.find(BarLayout.layout, id);
        if (id !== L.SEPARATOR && at !== null) {
            choose(L.key(at.zone, at.index));
            return;
        }
        if (BarLayout.layout.end.length + BarLayout.layout.center.length + BarLayout.layout.start.length >= L.MAX_ITEMS)
            return;
        choose(L.key("end", BarLayout.layout.end.length));
        BarLayout.add(id, "end");
    }
    // Whether the all-islands text has `field` set ("theme" and "mono": the font).
    function allText(field) {
        var t = BarLayout.allText;
        return field === "mono" ? t.font === L.MONO : (field === "theme" ? t.font === null : t[field] === true);
    }
    function allSized(size) {
        var n = 0, on = 0;
        for (var i = 0; i < order.length; i++) {
            if (order[i].id === L.SEPARATOR)
                continue;
            n++;
            if (L.sizeOf(BarLayout.layout[order[i].zone][order[i].index]) === size)
                on++;
        }
        return n > 0 && on === n;
    }

    onShownChanged: {
        if (shown) {
            if (BarLayout.selected === "" && order.length > 0)
                BarLayout.selected = order[0].key;
            page = "island";
            panelBox.forceActiveFocus();
        }
    }

    // The dim: everything below the bar, a click on it closes the dialog.
    Rectangle {
        id: scrim
        anchors.fill: parent
        color: "black"
        opacity: win.shown ? 0.38 : 0
        Behavior on opacity {
            enabled: !Settings.reduceMotion
            NumberAnimation { duration: Motion.base; easing.type: Motion.easing }
        }
        MouseArea {
            anchors.fill: parent
            onClicked: win.close()
        }
    }

    Rectangle {
        id: panelBox
        // Above the edit panel at the bottom of the screen.
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: Theme.gapOut * 2
        width: Math.min(parent.width - Theme.gapOut * 4, Theme.fontSize * 66)
        height: Math.min(parent.height - Theme.gapOut * 2 - Theme.fontSize * 13, Theme.fontSize * 46)
        color: Theme.surface
        radius: Theme.radius
        border.color: Theme.accent
        border.width: Theme.borderWidth
        focus: true
        visible: win.shown
        Accessible.role: Accessible.Dialog
        Accessible.name: "Bar islands"
        Keys.onEscapePressed: win.close()
        // A click inside must not reach the scrim.
        MouseArea {
            anchors.fill: parent
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: Theme.gapOut
            spacing: Theme.gapIn

            RowLayout {
                Layout.fillWidth: true
                UqText {
                    text: "Bar islands"
                    baseBold: true
                    Layout.fillWidth: true
                }
                UqText {
                    text: "Changes are saved as you make them."
                    color: Theme.textMuted
                }
                MenuRow {
                    text: "Close"
                    detail: "Esc"
                    onActivated: win.close()
                }
            }
            UqText {
                visible: BarLayout.error !== ""
                text: BarLayout.error
                color: Theme.error
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: Theme.gapOut

                // --- left: what is on the bar, and what can be added ---
                Flickable {
                    id: leftFlick
                    Layout.preferredWidth: Theme.fontSize * 22
                    Layout.fillHeight: true
                    clip: true
                    contentWidth: width
                    contentHeight: leftCol.implicitHeight
                    boundsBehavior: Flickable.StopAtBounds

                    ColumnLayout {
                        id: leftCol
                        width: leftFlick.width
                        spacing: 2

                        MenuRow {
                            Layout.fillWidth: true
                            text: "All islands"
                            detail: win.page === "all" ? "✓" : ""
                            Accessible.checkable: true
                            Accessible.checked: win.page === "all"
                            onActivated: win.page = "all"
                        }
                        UqText {
                            text: "On the bar"
                            color: Theme.textMuted
                            Layout.topMargin: Theme.gapIn
                        }
                        UqText {
                            visible: win.order.length === 0
                            text: "No islands. Add one below."
                            color: Theme.textMuted
                        }
                        Repeater {
                            model: win.order
                            delegate: MenuRow {
                                required property var modelData
                                readonly property bool chosen: win.page === "island" && BarLayout.selected === modelData.key
                                Layout.fillWidth: true
                                text: L.name(modelData.id)
                                detail: chosen ? "✓" : win.zoneNames[modelData.zone]
                                color: chosen ? Theme.surfaceAlt : (lit ? Theme.surfaceAlt : "transparent")
                                Accessible.checkable: true
                                Accessible.checked: chosen
                                onActivated: win.choose(modelData.key)
                            }
                        }
                        UqText {
                            text: "Island types"
                            color: Theme.textMuted
                            Layout.topMargin: Theme.gapIn * 2
                        }
                        Repeater {
                            model: L.ISLANDS
                            delegate: MenuRow {
                                required property var modelData
                                readonly property bool onBar: modelData.id !== L.SEPARATOR && L.placed(BarLayout.layout, modelData.id)
                                Layout.fillWidth: true
                                text: modelData.name
                                detail: onBar ? "On the bar" : "Add"
                                Accessible.name: onBar ? modelData.name + ", on the bar" : "Add " + modelData.name
                                onActivated: win.pickType(modelData.id)
                            }
                        }
                    }
                }

                Rectangle {
                    Layout.fillHeight: true
                    width: Theme.borderWidth
                    color: Theme.border
                }

                // --- right: the chosen island's settings, or every island's ---
                Flickable {
                    id: rightFlick
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    contentWidth: width
                    contentHeight: rightCol.implicitHeight
                    boundsBehavior: Flickable.StopAtBounds

                    ColumnLayout {
                        id: rightCol
                        width: rightFlick.width
                        spacing: 2

                        UqText {
                            visible: win.page === "island" && win.selTarget === null
                            text: "Choose an island on the left to set it up, or add one."
                            color: Theme.textMuted
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        IslandForm {
                            id: form
                            visible: win.page === "island" && win.selTarget !== null
                            autoFocus: false
                            formWidth: rightFlick.width
                            target: win.selTarget
                            anchorItem: win.selSlot
                            Layout.fillWidth: true
                            Layout.preferredHeight: visible ? implicitHeight : 0
                            Connections {
                                target: BarLayout
                                function onSelectedChanged() { form.reset(); }
                            }
                        }

                        ColumnLayout {
                            id: allBox
                            visible: win.page === "all"
                            spacing: 2
                            Layout.fillWidth: true

                            UqText {
                                text: "All islands"
                                baseBold: true
                            }
                            UqText {
                                text: "Settings for every island. An island can still have its own text and size."
                                color: Theme.textMuted
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                            }

                            UqText {
                                text: "Size"
                                color: Theme.textMuted
                                Layout.topMargin: Theme.gapIn
                            }
                            Flow {
                                spacing: 2
                                Layout.fillWidth: true
                                Repeater {
                                    model: ["icon", "small", "medium", "large"]
                                    delegate: MenuRow {
                                        required property string modelData
                                        text: L.SIZE_NAMES[modelData]
                                        detail: win.allSized(modelData) ? "✓" : ""
                                        Accessible.checkable: true
                                        Accessible.checked: win.allSized(modelData)
                                        onActivated: BarLayout.setSizeAll(modelData)
                                    }
                                }
                            }
                            UqText {
                                text: "Islands that don't fit the screen step down a size on their own."
                                color: Theme.textMuted
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Layout.topMargin: Theme.gapIn
                                UqText {
                                    text: "Text"
                                    color: Theme.textMuted
                                    Layout.fillWidth: true
                                }
                                MenuRow {
                                    visible: !L.isPlainText(BarLayout.allText)
                                    text: "Use the theme's"
                                    onActivated: BarLayout.setAllText(null)
                                }
                            }
                            UqText {
                                text: "Every island that has no text of its own uses this."
                                color: Theme.textMuted
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                            }
                            Flow {
                                spacing: 2
                                Layout.fillWidth: true
                                MenuRow {
                                    text: "Theme font"
                                    detail: win.allText("theme") ? "✓" : ""
                                    Accessible.checkable: true
                                    Accessible.checked: win.allText("theme")
                                    onActivated: BarLayout.setAllText({ font: null })
                                }
                                MenuRow {
                                    text: "Monospace"
                                    detail: win.allText("mono") ? "✓" : ""
                                    Accessible.checkable: true
                                    Accessible.checked: win.allText("mono")
                                    onActivated: BarLayout.setAllText({ font: L.MONO })
                                }
                                MenuRow {
                                    text: "Bold"
                                    textItem.baseBold: true
                                    detail: win.allText("bold") ? "✓" : ""
                                    Accessible.checkable: true
                                    Accessible.checked: win.allText("bold")
                                    onActivated: BarLayout.setAllText({ bold: !win.allText("bold") })
                                }
                                MenuRow {
                                    text: "Italic"
                                    textItem.font.italic: true
                                    detail: win.allText("italic") ? "✓" : ""
                                    Accessible.checkable: true
                                    Accessible.checked: win.allText("italic")
                                    onActivated: BarLayout.setAllText({ italic: !win.allText("italic") })
                                }
                                MenuRow {
                                    text: "All capitals"
                                    detail: win.allText("caps") ? "✓" : ""
                                    Accessible.checkable: true
                                    Accessible.checked: win.allText("caps")
                                    onActivated: BarLayout.setAllText({ caps: !win.allText("caps") })
                                }
                            }
                            UqText {
                                text: "Text size"
                                color: Theme.textMuted
                                Layout.topMargin: Theme.gapIn
                            }
                            Flow {
                                spacing: 2
                                Layout.fillWidth: true
                                Repeater {
                                    model: [70, 80, 90, 100, 110, 120, 130, 140, 150]
                                    delegate: MenuRow {
                                        required property int modelData
                                        text: modelData + "%"
                                        detail: BarLayout.allText.size === modelData ? "✓" : ""
                                        onActivated: BarLayout.setAllText({ size: modelData })
                                    }
                                }
                            }

                            MenuRow {
                                Layout.fillWidth: true
                                Layout.topMargin: Theme.gapIn
                                text: "Clear every island's own text"
                                detail: "They all follow the text above"
                                onActivated: BarLayout.setTextAll(null)
                            }

                            UqText {
                                text: "The bar"
                                color: Theme.textMuted
                                Layout.topMargin: Theme.gapIn * 2
                            }
                            MenuRow {
                                Layout.fillWidth: true
                                text: "Reset the bar"
                                detail: "Islands as on a new setup"
                                onActivated: BarLayout.reset()
                            }
                        }
                    }
                }
            }
        }
    }
}
