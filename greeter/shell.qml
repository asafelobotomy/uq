pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Hyprland
import Quickshell.Services.Greetd
import "core"
import "auth"
import "logic/greeter.js" as G
import "logic/tokens.js" as Tokens

// uq-greeter: a Quickshell config of its own, run as the `greeter` user by
// /usr/lib/uq/uq-greeter-session inside a Hyprland with a fixed config (spec phase 5 §2).
// It reads only the paths in greeter.js (§2.2) and never a home directory. core/, logic/ and
// auth/ are symlinks to ../shell/…: Quickshell refuses imports from outside the config directory.
//
// The decisions live in logic/greeter.js; the conversation in Auth.qml; the form in Greeter.qml.
// This file owns the files, processes and windows. Nothing is logged except fixed strings.
ShellRoot {
    id: root

    property var config: G.greeterConfig("")
    property var stateData: G.parseState("")
    // A user whose session just ended straight away (kept selected), else "".
    property string endedUser: ""
    property var users: []
    property bool usersDone: false
    property var sessions: []
    property bool sessionsDone: false
    property bool started: false
    // The monitor with the login form, chosen once at start.
    property string mainName: ""
    // Where uq-greeter-session wants "ready" and "launched" written (its crash detection).
    readonly property string statusPath: Quickshell.env("UQ_GREETER_STATUS") || ""

    Component.onCompleted: {
        Quickshell.watchFiles = false;
        config = G.greeterConfig(readFile(G.CONFIG_PATH));
        applyTheme();
        var st = G.parseState(readFile(G.STATE_PATH));
        endedUser = G.endedEarly(st, Date.now());
        stateData = G.clearLaunch(st);
        sessionDump.running = true;
        userPaths.running = true;
    }

    // --- files ---
    Component {
        id: readerC
        FileView {
            blockLoading: true
            printErrors: false
        }
    }

    function readFile(path) {
        var v = readerC.createObject(root, { path: path });
        var t = null;
        try { t = v.text(); } catch (e) { t = null; }
        v.destroy();
        return t;
    }

    function applyTheme() {
        var o = Tokens.parse(readFile(G.tokensPath(config, false)));
        if (o === null)
            o = Tokens.parse(readFile(G.tokensPath(config, true)));
        if (o !== null)
            Theme.apply(o);
    }

    FileView {
        id: stateFile
        path: G.STATE_PATH
        preload: false
        printErrors: false
        blockWrites: true
    }

    FileView {
        id: statusFile
        path: root.statusPath
        preload: false
        printErrors: false
        blockWrites: true
    }

    function writeStatus(word) {
        if (statusPath !== "")
            statusFile.setText(word + "\n");
    }

    // --- sessions: every /usr/share/wayland-sessions/*.desktop, as records for greeter.js ---
    Process {
        id: sessionDump
        property bool didStart: false
        onStarted: didStart = true
        command: ["sh", "-c", "for f in " + G.SESSIONS_DIR + "/*.desktop; do [ -f \"$f\" ] || continue; "
                  + "printf '\\036%s\\037' \"${f##*/}\"; head -c 65536 \"$f\"; done"]
        stdout: StdioCollector {
            onStreamFinished: {
                root.sessions = G.sessionList(G.parseSessionDump(text));
                root.sessionsDone = true;
                root.maybeStart();
            }
        }
        // sh failed to start: no sessions (the picker says so), but the form still shows.
        onRunningChanged: {
            if (!running && !didStart) {
                root.sessionsDone = true;
                root.maybeStart();
            }
        }
    }

    // --- users: AccountsService on the system bus, read-only, through busctl ---
    property var pendingPaths: []
    property var loadedUsers: []

    Process {
        id: userPaths
        property bool didStart: false
        onStarted: didStart = true
        command: ["busctl", "--system", "--json=short", "--timeout=5", "call", "org.freedesktop.Accounts",
                  "/org/freedesktop/Accounts", "org.freedesktop.Accounts", "ListCachedUsers"]
        stdout: StdioCollector {
            onStreamFinished: {
                root.pendingPaths = G.parseUserPaths(text);
                root.nextUser();
            }
        }
        // busctl missing or AccountsService not running: a user-name field instead.
        onExited: code => { if (code !== 0) root.usersFinished(); }
        onRunningChanged: if (!running && !didStart) root.usersFinished()
    }

    Process {
        id: userProps
        property string objectPath: ""
        // Exit and stdout arrive in either order; the next user starts once both are in (a
        // Process can't be restarted while it is still running).
        property bool gotExit: false
        property bool gotOut: false
        property string outText: ""
        function conclude() {
            if (!gotExit || !gotOut)
                return;
            gotExit = false;
            gotOut = false;
            var u = G.parseUserProps(outText);
            if (u)
                root.loadedUsers = root.loadedUsers.concat([u]);
            root.nextUser();
        }
        command: ["busctl", "--system", "--json=short", "--timeout=5", "call", "org.freedesktop.Accounts", objectPath,
                  "org.freedesktop.DBus.Properties", "GetAll", "s", "org.freedesktop.Accounts.User"]
        stdout: StdioCollector {
            onStreamFinished: {
                userProps.outText = text;
                userProps.gotOut = true;
                userProps.conclude();
            }
        }
        onExited: {
            gotExit = true;
            conclude();
        }
    }

    function nextUser() {
        if (pendingPaths.length === 0) {
            usersFinished();
            return;
        }
        userProps.objectPath = pendingPaths[0];
        pendingPaths = pendingPaths.slice(1);
        userProps.running = true;
    }

    function usersFinished() {
        if (usersDone)
            return;
        users = G.visibleUsers(loadedUsers);
        usersDone = true;
        maybeStart();
    }

    // Users and sessions are both in: show the form.
    function maybeStart() {
        if (started || !usersDone || !sessionsDone)
            return;
        started = true;
        var names = [];
        for (var i = 0; i < Quickshell.screens.length; i++)
            names.push(Quickshell.screens[i].name);
        mainName = G.mainScreen(names, Hyprland.focusedMonitor ? Hyprland.focusedMonitor.name : "");
        writeStatus("ready");
    }

    // A screen added or removed before the form showed is covered by maybeStart; afterwards the
    // form stays where it is unless its screen goes away.
    Connections {
        target: Quickshell
        function onScreensChanged() {
            if (!root.started)
                return;
            var names = [];
            for (var i = 0; i < Quickshell.screens.length; i++)
                names.push(Quickshell.screens[i].name);
            if (names.indexOf(root.mainName) < 0)
                root.mainName = G.mainScreen(names, "");
        }
    }

    // --- the conversation ---
    Auth {
        id: conversation
        greetd: Greetd
        onLaunching: (user, sessionId) => {
            stateFile.setText(G.serializeState(G.recordLaunch(root.stateData, user, sessionId, Date.now())));
        }
        onLaunched: {
            root.writeStatus("launched");
            Qt.quit();
        }
    }

    // --- power, through logind (systemctl) ---
    Process {
        id: powerProc
        property string action: ""
        command: G.powerCommand(action) || ["true"]
        stderr: StdioCollector { id: powerErr }
        onExited: code => {
            if (code !== 0)
                conversation.notify(G.powerFailure(powerProc.action, powerErr.text));
        }
    }

    function power(action) {
        if (powerProc.running || G.powerCommand(action) === null)
            return;
        powerProc.action = action;
        powerProc.running = true;
    }

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
    }

    // --- one window per screen: wallpaper (or the theme's bg) and the clock; the form on one ---
    Variants {
        model: Quickshell.screens

        PanelWindow {
            id: win
            required property var modelData
            readonly property bool isMain: root.started && modelData.name === root.mainName

            screen: modelData
            color: Theme.bg
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Top
            WlrLayershell.namespace: "uq-greeter"
            WlrLayershell.keyboardFocus: isMain ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            // The greeter decodes the wallpaper itself; nothing root-side ever does (§2.2).
            Image {
                anchors.fill: parent
                source: "file://" + G.WALLPAPER_PATH
                sourceSize.width: win.width
                sourceSize.height: win.height
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                cache: false
                visible: status === Image.Ready
            }

            Clock {
                visible: !win.isMain
                anchors.centerIn: parent
                date: clock.date
            }

            Loader {
                anchors.fill: parent
                focus: true
                active: win.isMain
                sourceComponent: Greeter {
                    id: form
                    auth: conversation
                    users: root.users
                    hideUsers: root.config.hideUsers
                    sessions: root.sessions
                    stateData: root.stateData
                    reduceMotion: root.config.reduceMotion
                    date: clock.date
                    focus: true
                    onPowerRequested: action => root.power(action)
                    Component.onCompleted: {
                        start(root.endedUser);
                        if (root.endedUser !== "")
                            conversation.notify({ text: G.ENDED_EARLY_TEXT, error: true });
                    }
                }
            }
        }
    }
}
