pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell.Hyprland
import "../core"
import "../logic/quick.js" as Q
import "../logic/quickui.js" as U
import "../logic/failure.js" as F

// Quick settings: theme picker, dark/light, the dark/light schedule, and the focused workspace's
// layout. All of it lives in uqd, reached through the bridge; it refetches when the panel opens
// and on ThemeChanged / LayoutChanged. A failed refetch is ignored quietly (uqd may be down, and
// the group is greyed out while it is); a failed switch is logged and says why in `report`.
ColumnLayout {
    id: root

    // Set by QuickSettings: the window is open.
    property bool active: false

    property var themes: []
    property string themeName: ""
    property string variant: ""
    property var schedule: null
    property var layouts: ({})
    // Why the last theme, schedule or layout switch failed (failure.js); "" when none did.
    property string report: ""

    readonly property var workspace: Hyprland.focusedWorkspace
    readonly property string layout: U.workspaceLayout(layouts, workspace, U.defaultLayout(Settings.values))
    readonly property var location: Settings.values.weather_location || null
    readonly property var nextSchedule: schedule ? Q.nextSchedule(schedule, location) : null

    spacing: Theme.gapIn * 2
    enabled: Bridge.uqdUp
    opacity: enabled ? 1 : 0.5

    function refreshTheme() {
        // Names come from ListThemeInfo; an older uqd has only ListThemes, so show the ids then.
        Bridge.call("ListThemeInfo", []).then(function (v) {
            root.themes = U.themeInfoList(v);
        }).catch(function () {
            Bridge.call("ListThemes", []).then(function (v) {
                root.themes = U.themeList(v).map(function (id) {
                    return { id: id, name: id.length > 24 ? id.substring(0, 24) : id };
                });
            }).catch(function () {});
        });
        Bridge.call("GetTheme", []).then(function (v) {
            var t = U.themeReply(v);
            root.themeName = t ? t.name : "";
            root.variant = t ? t.variant : "";
        }).catch(function () {});
        Bridge.call("GetSchedule", []).then(function (s) {
            root.schedule = s && typeof s === "object" ? s : null;
        }).catch(function () {});
    }
    function refreshLayouts() {
        Bridge.call("GetLayouts", []).then(function (m) {
            if (m && typeof m === "object")
                root.layouts = m;
        }).catch(function () {});
    }
    function refresh() {
        refreshTheme();
        refreshLayouts();
    }

    function setTheme(name, variant) {
        if (name === "" || (name === themeName && variant === root.variant))
            return;
        report = "";
        Bridge.call("SetTheme", [name, variant]).then(root.refreshTheme)
            .catch(root.failed("Couldn't switch the theme"));
    }
    // uqd emits no event for a schedule change, so refetch after a successful set.
    function toggleSchedule() {
        var next = nextSchedule;
        if (next === null)
            return;
        report = "";
        Bridge.call("SetSchedule", [JSON.stringify(next)]).then(root.refreshTheme)
            .catch(root.failed("Couldn't change the schedule"));
    }
    function setLayout(l) {
        var sel = U.workspaceSelector(workspace);
        if (sel === "" || l === layout)
            return;
        report = "";
        Bridge.call("SetLayout", [sel, l]).then(root.refreshLayouts)
            .catch(root.failed("Couldn't change the layout"));
    }

    // A catch handler for a switch: log the reason and show it under the group.
    function failed(what) {
        return function (e) {
            console.warn("uq: " + what + ": " + e);
            root.report = F.failureText(what, e);
        };
    }

    onActiveChanged: {
        if (active)
            refresh();
        else
            report = "";
    }

    Connections {
        target: Bridge
        function onThemeChanged() { if (root.active) root.refreshTheme(); }
        function onLayoutChanged(ws, layout) { if (root.active) root.refreshLayouts(); }
        function onUqdUpChanged() { if (root.active && Bridge.uqdUp) root.refresh(); }
    }

    UqText {
        text: "Appearance"
        color: Theme.textMuted
        font.bold: true
    }

    // --- theme picker ---
    Flow {
        visible: root.themes.length > 0
        Layout.fillWidth: true
        spacing: Theme.gapIn

        Repeater {
            model: root.themes
            delegate: Rectangle {
                id: chip
                required property var modelData
                readonly property bool current: modelData.id === root.themeName
                implicitWidth: Math.min(chipLabel.implicitWidth + Theme.gapIn * 4, 160)
                implicitHeight: 28
                radius: Theme.radius - 2
                color: current ? Theme.accent : (chipMouse.containsMouse ? Theme.overlay : Theme.surfaceAlt)
                activeFocusOnTab: true
                border.color: Theme.accent
                border.width: activeFocus && !current ? 1 : 0
                Accessible.role: Accessible.RadioButton
                Accessible.checkable: true
                Accessible.checked: current
                Accessible.name: "Theme " + modelData.name
                Accessible.onPressAction: chip.choose()
                Keys.onSpacePressed: chip.choose()
                Keys.onReturnPressed: chip.choose()

                function choose() {
                    root.setTheme(chip.modelData.id, root.variant !== "" ? root.variant : "dark");
                }

                UqText {
                    id: chipLabel
                    anchors.fill: parent
                    anchors.leftMargin: Theme.gapIn * 2
                    anchors.rightMargin: Theme.gapIn * 2
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    maximumLineCount: 1
                    text: chip.modelData.name
                    color: chip.current ? Theme.accentText : Theme.text
                }
                MouseArea {
                    id: chipMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: chip.choose()
                }
            }
        }
    }

    // --- dark / light ---
    ToggleRow {
        visible: root.themeName !== ""
        glyph: "🌙"
        label: "Dark mode"
        on: root.variant === "dark"
        onToggled: root.setTheme(root.themeName, U.otherVariant(root.variant))
    }

    // --- schedule ---
    ToggleRow {
        visible: root.schedule !== null
        enabled: root.nextSchedule !== null
        opacity: enabled ? 1 : 0.5
        glyph: "🕒"
        label: "Schedule: " + Q.scheduleLabel(root.schedule)
        on: root.schedule !== null && root.schedule.mode !== "off"
        onToggled: root.toggleSchedule()
    }
    UqText {
        visible: root.schedule !== null && root.nextSchedule === null
        Layout.fillWidth: true
        text: "Set a weather city first"
        color: Theme.textMuted
    }

    // --- focused workspace layout ---
    RowLayout {
        visible: root.workspace !== null
        Layout.fillWidth: true
        spacing: Theme.gapIn

        UqText {
            text: "Layout"
            color: Theme.textMuted
            Layout.preferredWidth: 56
        }
        Repeater {
            model: Q.LAYOUTS
            delegate: Rectangle {
                id: seg
                required property string modelData
                readonly property bool current: root.layout === modelData
                Layout.fillWidth: true
                implicitHeight: 28
                radius: Theme.radius - 2
                color: current ? Theme.accent : (segMouse.containsMouse ? Theme.overlay : Theme.surfaceAlt)
                activeFocusOnTab: true
                border.color: Theme.accent
                border.width: activeFocus && !current ? 1 : 0
                Accessible.role: Accessible.RadioButton
                Accessible.checkable: true
                Accessible.checked: current
                Accessible.name: "Workspace layout " + modelData
                Accessible.onPressAction: root.setLayout(seg.modelData)
                Keys.onSpacePressed: root.setLayout(seg.modelData)
                Keys.onReturnPressed: root.setLayout(seg.modelData)

                UqText {
                    anchors.fill: parent
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    text: seg.modelData.charAt(0).toUpperCase() + seg.modelData.substring(1)
                    color: seg.current ? Theme.accentText : Theme.text
                }
                MouseArea {
                    id: segMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: root.setLayout(seg.modelData)
                }
            }
        }
    }

    UqText {
        objectName: "themeReport"
        visible: root.report !== ""
        Layout.fillWidth: true
        text: root.report
        color: Theme.error
        wrapMode: Text.Wrap
    }
}
