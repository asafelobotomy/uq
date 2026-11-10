pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import "../core"
import "../logic/quickui.js" as U

// Quick settings: switch profiles. "Normal" (Back to normal) first, then one chip per profile.
// The list lives in uqd, reached through the bridge; it refetches when the panel opens, on
// ProfileChanged and when uqd comes back. Hidden when there are no profiles and greyed out while
// uqd is down. A failed refetch is ignored quietly; a failed switch says why in the report line.
// Profile names are untrusted: UqText only, one line, elided.
ColumnLayout {
    id: root

    // Set by QuickSettings: the window is open.
    property bool active: false

    property var profiles: []
    readonly property string current: U.activeProfile(profiles)
    // What the last switch reported (kept keys, notices, or why it failed); cleared when the
    // panel closes.
    property string report: ""

    visible: profiles.length > 0
    spacing: Theme.gapIn * 2
    enabled: Bridge.uqdUp
    opacity: enabled ? 1 : 0.5

    function refresh() {
        Bridge.call("ListProfiles", []).then(function (v) {
            // Reassigning rebuilds every chip and drops keyboard focus, so only on a real change.
            var next = U.profileList(v);
            if (!U.sameProfiles(next, root.profiles))
                root.profiles = next;
        }).catch(function () {});
    }

    // id "" is Back to normal.
    function choose(id) {
        if (id === current)
            return;
        var call = id === "" ? Bridge.call("ProfileOff", []) : Bridge.call("ApplyProfile", [id]);
        call.then(function (r) {
            root.report = U.profileReportText(r);
            root.refresh();
        }).catch(function (e) {
            root.report = U.profileErrorText(e);
        });
    }

    onActiveChanged: {
        if (active)
            refresh();
        else
            report = "";
    }

    Connections {
        target: Bridge
        function onProfileChanged(id) { if (root.active) root.refresh(); }
        function onUqdUpChanged() { if (root.active && Bridge.uqdUp) root.refresh(); }
    }

    UqText {
        text: "Profile"
        color: Theme.textMuted
        font.bold: true
    }

    Flow {
        Layout.fillWidth: true
        spacing: Theme.gapIn

        Repeater {
            model: [{ id: "", name: "Normal" }].concat(root.profiles)
            delegate: Rectangle {
                id: chip
                required property var modelData
                readonly property bool current: modelData.id === root.current
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
                Accessible.name: modelData.id === "" ? "Back to normal" : "Profile " + modelData.name
                Accessible.onPressAction: root.choose(chip.modelData.id)
                Keys.onSpacePressed: root.choose(chip.modelData.id)
                Keys.onReturnPressed: root.choose(chip.modelData.id)

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
                    onClicked: root.choose(chip.modelData.id)
                }
            }
        }
    }

    UqText {
        visible: root.report !== ""
        Layout.fillWidth: true
        text: root.report
        color: Theme.textMuted
        wrapMode: Text.Wrap
    }
}
