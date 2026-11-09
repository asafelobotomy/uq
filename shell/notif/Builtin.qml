pragma Singleton
pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Services.Notifications
import "../logic/notif.js" as N
import "../logic/sanitize.js" as S

// uQ's own notification server (spec §2.5). The NotificationServer sits in a LazyLoader, so
// org.freedesktop.Notifications is taken only while NotifBackend sets `wanted` (the name has no
// other owner). The rules (timeouts, DnD, caps) live in logic/sanitize.js; this owns the store,
// the popups and their deadlines. Notification content is untrusted: `latest` is cleaned here and
// bodies reach the screen only through notif/builtin/NotifBody.qml.
QtObject {
    id: root

    // --- public state ---
    property bool wanted: false                     // set by NotifBackend
    readonly property bool registered: loader.item !== null
    property bool dnd: false                        // in memory only
    property int count: 0                           // arrivals since the centre opened, ≤ notifications
    property var latest: null                       // {appIcon, appName, summary} or null
    property var notifications: []                  // Notification objects, newest first, ≤ MAX_KEEP
    property var popups: []                         // newest first, ≤ MAX_POPUPS

    // --- internals ---
    // {notification id: deadlineMs} for each popup that times out; popupTimer expires the overdue.
    property var deadlines: ({})
    // Notifications updated in place by a replacement (Notify with replaces_id), refreshed once
    // on the next turn however many of their properties changed.
    property var replaced: []

    function viewOf(n) {
        if (!n)
            return null;
        return {
            appIcon: typeof n.appIcon === "string" ? n.appIcon : "",
            appName: N.cleanText(n.appName, N.MAX_APP),
            summary: N.cleanText(n.summary, N.MAX_SUMMARY)
        };
    }

    function without(list, n) {
        return list.filter(function (x) { return x !== n; });
    }

    function dropDeadline(n) {
        var ds = deadlines;
        if (ds[n.id] === undefined)
            return;
        delete ds[n.id];
        deadlines = ds;
    }

    // `latest` and `count` first, so a handler of notificationsChanged already sees them. Unread
    // can't outnumber what is kept, so a dismissal or the store cap lowers count too.
    function setStore(list) {
        latest = viewOf(list.length > 0 ? list[0] : null);
        if (count > list.length)
            count = list.length;
        notifications = list;
    }

    // Removes n from everything this holds; it does not close it with the sender.
    function forget(n) {
        if (notifications.indexOf(n) >= 0)
            setStore(without(notifications, n));
        dropDeadline(n);
        if (popups.indexOf(n) >= 0)
            popups = without(popups, n);
    }

    function add(n) {
        n.tracked = true;
        var popup = S.popupAllowed(dnd, n.urgency);
        // A transient notification held back by DnD would never get the popup that removes it.
        if (!popup && n.transient) {
            n.expire();
            return;
        }
        n.closed.connect(function () { root.forget(n); });
        // Quickshell updates a replaced notification in place and does not emit notification()
        // again, so its changes are picked up here.
        var onChange = function () { root.markReplaced(n); };
        n.summaryChanged.connect(onChange);
        n.bodyChanged.connect(onChange);
        n.appNameChanged.connect(onChange);
        n.appIconChanged.connect(onChange);
        var store = [n].concat(notifications);
        var extra = S.trimKept(store.length);
        var gone = extra > 0 ? store.splice(store.length - extra, extra) : [];
        count++;
        setStore(store);
        if (popup)
            showPopup(n);
        gone.forEach(function (g) {
            root.forget(g);
            g.expire();
        });
    }

    function markReplaced(n) {
        if (replaced.indexOf(n) >= 0)
            return;
        replaced = replaced.concat([n]);
        Qt.callLater(root.refreshReplaced);
    }

    // A replacement is shown like a new arrival (front of the store, popup and deadline again),
    // but it is the same notification, so `count` is unchanged.
    function refreshReplaced() {
        var list = replaced;
        replaced = [];
        list.forEach(function (n) {
            if (root.notifications.indexOf(n) < 0)
                return;
            root.setStore([n].concat(root.without(root.notifications, n)));
            if (S.popupAllowed(root.dnd, n.urgency))
                root.showPopup(n);
        });
    }

    function showPopup(n) {
        dropDeadline(n);
        var ps = [n].concat(without(popups, n));
        var pushed = ps.length > S.MAX_POPUPS ? ps.splice(S.MAX_POPUPS) : [];
        popups = ps;
        var ms = S.timeoutMs(n.expireTimeout, n.urgency);
        if (ms > 0) {
            var ds = deadlines;
            ds[n.id] = Date.now() + ms;
            deadlines = ds;
            if (!popupTimer.running)
                popupTimer.start();
        }
        pushed.forEach(function (p) { root.expirePopup(p); });
    }

    // The popup goes (timed out, pushed out or closed by the user); the notification stays in
    // the store unless it is transient.
    function expirePopup(n) {
        if (!n)
            return;
        dropDeadline(n);
        if (popups.indexOf(n) >= 0)
            popups = without(popups, n);
        if (n.transient && notifications.indexOf(n) >= 0) {
            forget(n);
            n.expire();
        }
    }

    function expirePopups() {
        var now = Date.now();
        var ds = deadlines;
        var due = popups.filter(function (p) { return ds[p.id] !== undefined && ds[p.id] <= now; });
        due.forEach(function (p) { root.expirePopup(p); });
        if (Object.keys(deadlines).length === 0)
            popupTimer.stop();
    }

    function markRead() {
        count = 0;
    }

    function dismiss(n) {
        if (!n || notifications.indexOf(n) < 0)
            return;
        forget(n);
        n.dismiss();
    }

    function clearAll() {
        var all = notifications;
        setStore([]);
        popups = [];
        deadlines = ({});
        popupTimer.stop();
        all.forEach(function (n) { n.dismiss(); });
    }

    // Runs one of n's own actions; a non-resident notification is then closed by Quickshell.
    function invoke(n, identifier) {
        if (!n || notifications.indexOf(n) < 0)
            return;
        var list = n.actions || [];
        for (var i = 0; i < list.length; i++) {
            if (list[i] && list[i].identifier === identifier) {
                list[i].invoke();
                return;
            }
        }
    }

    // The server (and every Notification it tracked) goes with the loader's item.
    onWantedChanged: {
        if (wanted)
            return;
        setStore([]);
        popups = [];
        deadlines = ({});
        replaced = [];
        count = 0;
        popupTimer.stop();
    }

    property LazyLoader loader: LazyLoader {
        active: root.wanted

        NotificationServer {
            keepOnReload: false
            persistenceSupported: true
            bodySupported: true
            bodyMarkupSupported: true
            bodyHyperlinksSupported: false
            bodyImagesSupported: false
            actionsSupported: true
            imageSupported: true
            inlineReplySupported: false

            onNotification: n => root.add(n)
        }
    }

    // One shared ticker for every popup deadline; runs only while a popup is timed.
    property Timer popupTimer: Timer {
        interval: 250
        repeat: true
        onTriggered: root.expirePopups()
    }
}
