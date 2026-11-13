import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets
import Quickshell.Services.Mpris
import "../../core"
import ".."

// Now playing (`media`): the playing (else the first) player. Icon: play/pause. Small: and
// next. Medium and Large: previous, play/pause, next and the track, scrolling when long. Hidden
// while no player is open (shown, empty, in edit mode). While this island is on the bar, the
// Status island leaves media to it. Track titles are untrusted: they only reach UqText.
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
    readonly property bool wide: size === "medium" || size === "large"

    shown: player !== null
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
        anchors.fill: parent
        spacing: 0

        Btn {
            visible: root.wide
            icon: "media-skip-backward"
            label: "Previous track"
            can: !!root.player && root.player.canGoPrevious
            onPressed: root.player.previous()
        }
        Btn {
            Layout.alignment: Qt.AlignHCenter
            Layout.fillWidth: root.isIcon
            icon: root.player && root.player.isPlaying ? "media-playback-pause" : "media-playback-start"
            label: root.player && root.player.isPlaying ? "Pause" : "Play"
            can: !!root.player && root.player.canTogglePlaying
            onPressed: root.player.togglePlaying()
        }
        Btn {
            visible: !root.isIcon
            icon: "media-skip-forward"
            label: "Next track"
            can: !!root.player && root.player.canGoNext
            onPressed: root.player.next()
        }
        Marquee {
            visible: root.wide && root.track !== ""
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.leftMargin: Theme.gapIn
            model: [root.track]
            delegate: UqText {
                required property var modelData
                text: modelData
                elide: Text.ElideNone
            }
        }
    }
}
