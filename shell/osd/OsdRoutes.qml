import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Services.Pipewire
import "../brightness"
import "../logic/quick.js" as Q

// The volume, mic and brightness keys, routed from Hyprland global shortcuts (base.lua binds
// `uq:vol-up` ... `uq:bright-down`, locked and repeating). No windows in here, so it also loads
// headless (tests/shell/bar_loads.sh). All act on `pressed`; key-repeat comes from Hyprland. A key
// is a no-op when its node or the node's audio is null. The OSD comes from OsdWatch, which sees
// the resulting level change.
Scope {
    id: root

    readonly property PwNode sink: Pipewire.defaultAudioSink
    readonly property PwNode source: Pipewire.defaultAudioSource

    // True once all six shortcuts exist with the names base.lua and the README use.
    readonly property bool routed: volUp.appid === "uq" && volUp.name === "vol-up"
        && volDown.appid === "uq" && volDown.name === "vol-down"
        && muteKey.appid === "uq" && muteKey.name === "mute"
        && micMuteKey.appid === "uq" && micMuteKey.name === "mic-mute"
        && brightUp.appid === "uq" && brightUp.name === "bright-up"
        && brightDown.appid === "uq" && brightDown.name === "bright-down"

    // Audio properties only update while the node is bound.
    PwObjectTracker {
        objects: [root.sink, root.source]
    }

    function volume(dir) {
        const a = sink ? sink.audio : null;
        if (!a)
            return;
        const next = Q.volumeStep(a.volume, dir);
        const unmute = dir > 0 && a.muted;
        // At 0% or 100% nothing changes, so OsdWatch sees no signal; show the level directly so
        // each key-repeat step still gets an OSD.
        if (next === Math.round(a.volume * 100) / 100 && !unmute)
            OsdRouter.show("volume", Math.round(next * 100), 100, a.muted);
        a.volume = next;
        if (unmute)
            a.muted = false;
    }

    function toggleMute(node) {
        const a = node ? node.audio : null;
        if (a)
            a.muted = !a.muted;
    }

    GlobalShortcut {
        id: volUp
        appid: "uq"
        name: "vol-up"
        description: "Raise the volume"
        onPressed: root.volume(1)
    }
    GlobalShortcut {
        id: volDown
        appid: "uq"
        name: "vol-down"
        description: "Lower the volume"
        onPressed: root.volume(-1)
    }
    GlobalShortcut {
        id: muteKey
        appid: "uq"
        name: "mute"
        description: "Mute or unmute the speakers"
        onPressed: root.toggleMute(root.sink)
    }
    GlobalShortcut {
        id: micMuteKey
        appid: "uq"
        name: "mic-mute"
        description: "Mute or unmute the microphone"
        onPressed: root.toggleMute(root.source)
    }
    GlobalShortcut {
        id: brightUp
        appid: "uq"
        name: "bright-up"
        description: "Raise the screen brightness"
        onPressed: Brightness.step(1)
    }
    GlobalShortcut {
        id: brightDown
        appid: "uq"
        name: "bright-down"
        description: "Lower the screen brightness"
        onPressed: Brightness.step(-1)
    }
}
