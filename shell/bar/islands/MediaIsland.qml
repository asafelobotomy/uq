import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets
import Quickshell.Services.Mpris
import "../../core"
import ".."

// Now playing (`media`): the playing (else the first) player's track, scrolling when long, with
// previous, play/pause and next. `compact` keeps just the play/pause button. Hidden while no
// player is open. While this island is on the bar, the Status island leaves media to it.
// Track titles are untrusted: they only reach UqText.
Island {
    id: root

    readonly property int iconSize: Theme.fontSize + 5
    readonly property var player: {
        var ps = Mpris.players.values;
        for (var i = 0; i < ps.length; i++)
            if (ps[i].isPlaying)
                return ps[i];
        return ps.length > 0 ? ps[0] : null;
    }
    readonly property string track: player ? [String(player.trackTitle || ""), String(player.trackArtist || "")]
        .filter(function (s) { return s !== ""; }).join(" – ") : ""

    shown: player !== null
    implicitWidth: row.implicitWidth + Theme.gapIn * 2
    Accessible.role: Accessible.Grouping
    Accessible.name: track !== "" ? "Now playing: " + track : "Media"

    component Btn: Item {
        id: btn
        property string icon: ""
        property string label: ""
        property bool can: true
        signal pressed()
        implicitWidth: root.iconSize + Theme.gapIn
        implicitHeight: root.iconSize + Theme.gapIn
        opacity: can ? 1 : 0.4
        Accessible.role: Accessible.Button
        Accessible.name: label
        Accessible.onPressAction: if (can) pressed()
        IconImage {
            anchors.centerIn: parent
            implicitSize: root.iconSize
            source: Quickshell.iconPath(btn.icon, true)
        }
        MouseArea {
            anchors.fill: parent
            enabled: btn.can
            onClicked: btn.pressed()
        }
    }

    RowLayout {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        spacing: Theme.gapIn

        Btn {
            visible: root.size !== "compact"
            icon: "media-skip-backward"
            label: "Previous track"
            can: !!root.player && root.player.canGoPrevious
            onPressed: root.player.previous()
        }
        Btn {
            icon: root.player && root.player.isPlaying ? "media-playback-pause" : "media-playback-start"
            label: root.player && root.player.isPlaying ? "Pause" : "Play"
            can: !!root.player && root.player.canTogglePlaying
            onPressed: root.player.togglePlaying()
        }
        Btn {
            visible: root.size !== "compact"
            icon: "media-skip-forward"
            label: "Next track"
            can: !!root.player && root.player.canGoNext
            onPressed: root.player.next()
        }
        Marquee {
            visible: root.size !== "compact" && root.track !== ""
            Layout.preferredWidth: Math.min(implicitStrip, Theme.fontSize * (root.size === "wide" ? 28 : 16))
            Layout.preferredHeight: root.iconSize
            readonly property real implicitStrip: probe.implicitWidth
            model: [root.track]
            delegate: UqText {
                required property var modelData
                text: modelData
                elide: Text.ElideNone
            }
            UqText {
                id: probe
                visible: false
                text: root.track
            }
        }
    }
}
