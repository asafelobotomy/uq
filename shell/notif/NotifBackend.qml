pragma Singleton
import QtQuick
import "../core"
import "adapters"
import "../logic/notif.js" as N

// One notification interface over noti-fi, the built-in server, swaync, mako and dunst (spec
// §2.5). `notif.backend` picks one (logic/notif.js chooseBackend); a missing daemon falls back to
// built-in with a warning; `auto` waits NOTIFI_GRACE_MS for noti-fi before settling on built-in,
// at startup and again whenever noti-fi drops.
// While the choice is "pending" nothing is available and the controls do nothing.
QtObject {
    id: root

    // --- settings ---
    readonly property var notifSettings: {
        var v = Settings.values;
        var n = v && typeof v === "object" ? v.notif : null;
        return n && typeof n === "object" ? n : ({});
    }
    readonly property string setting: typeof notifSettings.backend === "string" ? notifSettings.backend : "auto"
    readonly property bool showSummary: notifSettings.show_summary === true

    property bool graceOver: false

    readonly property var choice: N.chooseBackend(setting, {
        notifi: Notifi.available,
        graceOver: graceOver,
        detected: { swaync: swaync.detected, mako: mako.detected, dunst: dunst.detected }
    })

    // --- interface ---
    readonly property string backend: choice.backend
    readonly property string warning: choice.warning
    readonly property var source: {
        switch (backend) {
        case "notifi": return Notifi;
        case "builtin": return Builtin;
        case "swaync": return swaync;
        case "mako": return mako;
        case "dunst": return dunst;
        }
        return null;
    }
    readonly property bool available: backend === "builtin" ? Builtin.registered
        : (source !== null && source.available === true)
    readonly property int count: source !== null && typeof source.count === "number" ? source.count : 0
    readonly property var latest: source !== null && source.latest ? source.latest : null
    readonly property bool dnd: source !== null && source.dnd === true
    readonly property bool hasCentre: backend === "notifi" || backend === "builtin"
        || (source !== null && source.hasCentre === true)
    readonly property bool sharing: Bridge.sharing || (backend === "notifi" && Notifi.sharing)
    // The name's foreign owner while built-in is chosen ("another app" if it has no name), else "".
    readonly property string yieldedTo: {
        var o = Bridge.notifOwner;
        if (backend !== "builtin" || !o || o.known !== true || o.owner === null || o.self === true)
            return "";
        return o.name ? o.name : "another app";
    }

    function toggleCentre() {
        switch (backend) {
        case "notifi":
            Notifi.toggleCentre().catch(function () {});
            break;
        case "builtin":
            Surfaces.toggle("notifcentre", null, null);
            break;
        case "swaync":
        case "mako":
        case "dunst":
            source.toggleCentre();
            break;
        }
    }

    function setDnd(on) {
        switch (backend) {
        case "notifi":
            Notifi.setDnd(on).catch(function () {});
            break;
        case "builtin":
            Builtin.dnd = on;
            break;
        case "swaync":
        case "mako":
        case "dunst":
            source.setDnd(on);
            break;
        }
    }

    onWarningChanged: {
        if (warning !== "")
            console.warn("uq: notifications: " + warning);
    }

    // --- internals ---
    property Swaync swaync: Swaync {
        active: root.setting === "swaync"
    }
    property Mako mako: Mako {
        active: root.setting === "mako"
    }
    property Dunst dunst: Dunst {
        active: root.setting === "dunst"
    }

    // Built-in takes the name only when it has no owner (or the owner is this shell, which keeps
    // it once registered).
    property Binding wantBuiltin: Binding {
        target: Builtin
        property: "wanted"
        value: N.builtinShouldRegister(root.backend, Bridge.notifOwner)
    }

    // Ruling E: when noti-fi drops (crash or restart), wait a fresh grace window before falling
    // back to built-in, so built-in does not take the name the restarted daemon needs.
    property Connections notifiConn: Connections {
        target: Notifi
        function onAvailableChanged() {
            if (!Notifi.available) {
                root.graceOver = false;
                root.graceTimer.restart();
            }
        }
    }

    property Timer graceTimer: Timer {
        interval: N.NOTIFI_GRACE_MS
        running: true
        onTriggered: root.graceOver = true
    }
}
