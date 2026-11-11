import QtQuick
import "logic/greeter.js" as G

// The greetd conversation (spec phase 5 §2.3), driven by greeter.js. `greetd` is injected:
// greeter/shell.qml passes Quickshell's Greetd singleton, tests pass a fake with the same API.
//
// Quickshell.Services.Greetd (0.3), as used here:
//   Greetd.createSession(user), Greetd.respond(response), Greetd.cancelSession(),
//   Greetd.launch(command, environment, quit)
//   signals authMessage(message, error, responseRequired, echoResponse), authFailure(message),
//           readyToLaunch(), launched(), error(error)
// (Quickshell answers info and error messages itself; respond() is only for responseRequired.)
//
// Nothing here logs: not passwords, not PAM messages.
Item {
    id: auth

    property var greetd: null
    // The session to launch, {id, name, command} from greeter.js sessionList.
    property var session: null
    // greeter.js conversation state.
    property var conv: G.newConversation()

    readonly property string mode: G.fieldMode(conv)
    readonly property var message: conv.message
    readonly property bool busy: mode === "busy"
    readonly property string promptText: conv.prompt ? conv.prompt.text : ""
    readonly property string user: conv.user

    // Just before greetd is asked to start `sessionId` for `user` (the greeter saves its state).
    signal launching(string user, string sessionId)
    // greetd took the session: the greeter must exit now.
    signal launched()

    function apply(r) {
        conv = r.conv;
        for (var i = 0; i < r.actions.length; i++) {
            var a = r.actions[i];
            if (!greetd)
                return;
            if (a.create !== undefined) {
                greetd.createSession(a.create);
            } else if (a.respond !== undefined) {
                greetd.respond(a.respond);
            } else if (a.cancel) {
                greetd.cancelSession();
            } else if (a.launch) {
                if (!session || !session.command || session.command.length === 0) {
                    greetd.cancelSession();
                    var c = G.newConversation();
                    c.user = conv.user;
                    c.message = { text: "There is no session to start", error: true };
                    conv = c;
                    return;
                }
                launching(conv.user, session.id);
                greetd.launch(session.command, G.launchEnv(), false);
            }
        }
    }

    // Enter on the password field (or on a secret/visible prompt's field).
    function submit(userName, typed) { apply(G.submit(conv, userName, typed)); }
    function answer(text) { apply(G.answer(conv, text)); }
    function answerNew(first, second) { apply(G.answerNew(conv, first, second)); }
    // Esc, or another user chosen ("" keeps the current user).
    function cancel(userName) { apply(G.cancel(conv, userName || "")); }
    // A message shown while idle (e.g. "The session ended straight away", a power refusal).
    function notify(view) {
        var c = G.newConversation();
        c.user = conv.user;
        c.phase = conv.phase;
        c.pending = conv.pending;
        c.confirm = conv.confirm;
        c.prompt = conv.prompt;
        c.messages = conv.messages;
        c.message = view;
        conv = c;
    }

    Connections {
        target: auth.greetd
        function onAuthMessage(message, error, responseRequired, echoResponse) {
            auth.apply(G.onAuthMessage(auth.conv, message, error, responseRequired, echoResponse));
        }
        function onAuthFailure(message) { auth.apply(G.onAuthFailure(auth.conv)); }
        function onError(error) { auth.apply(G.onError(auth.conv)); }
        function onReadyToLaunch() { auth.apply(G.onReady(auth.conv)); }
        function onLaunched() {
            auth.apply(G.onLaunched(auth.conv));
            auth.launched();
        }
    }
}
